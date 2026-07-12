use std::collections::HashMap;

use crate::{Actor, Addr, Context, Handler, Message, Service, WeakSender, context::ContextID};

/// Enables global subscriptions and message distribution.
///
/// The `Broker` is a service actor that allows actors to publish and subscribe to messages by type.
///
/// # Example
/// ```
/// # use futures::future::join;
/// # use hannibal::{Broker, prelude::*};
/// #[derive(Clone, Message)]
/// struct Topic1(u32);
///
/// #[derive(Debug, Default, PartialEq)]
/// struct Subscribing(Vec<u32>);
///
/// impl Actor for Subscribing {
///     async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
///         // subscribe to `Topic1`
///         ctx.subscribe::<Topic1>().await?;
///         Ok(())
///     }
/// }
///
/// impl Handler<Topic1> for Subscribing {
///     async fn handle(&mut self, ctx: &mut Context<Self>, msg: Topic1) {
///         self.0.push(msg.0);
///         # if self.0.len() == 2 {
///         #     ctx.stop().unwrap()
///         # }
///     }
/// }
///
/// # #[hannibal::main]
/// # async fn main() {
/// let mut subscriber1 = Subscribing::default().spawn_owning();
/// let mut subscriber2 = Subscribing::default().spawn_owning();
/// # subscriber1.ping().await.unwrap();
/// # subscriber2.ping().await.unwrap();
///
/// let broker = Broker::from_registry().await;
/// broker.publish(Topic1(42)).await.unwrap();
/// broker.publish(Topic1(23)).await.unwrap();
///
/// assert_eq!(subscriber1.join().await, Some(Subscribing(vec![42, 23])));
/// assert_eq!(subscriber2.join().await, Some(Subscribing(vec![42, 23])));
/// # }
/// ```
pub struct Broker<T: Message<Response = ()>> {
    subscribers: HashMap<ContextID, WeakSender<T>>,
    external_senders: Vec<async_channel::Sender<T>>,
}

impl<T: Message<Response = ()> + Clone> Broker<T> {
    /// Publishes a message to all subscribers.
    pub async fn publish(topic: T) -> crate::error::Result<()> {
        Self::from_registry().await.publish(topic).await
    }

    /// Tries to publish a message to the broker.
    pub async fn try_publish(topic: T) -> Option<crate::error::Result<()>> {
        if let Some(broker) = Self::try_from_registry() {
            Some(broker.publish(topic).await)
        } else {
            None
        }
    }

    /// Subscribes to messages of the given type.
    pub async fn subscribe(sender: WeakSender<T>) -> crate::error::Result<()> {
        Self::from_registry().await.subscribe(sender).await
    }
}

impl<T: Message<Response = ()>> Default for Broker<T> {
    fn default() -> Self {
        Broker {
            subscribers: Default::default(),
            external_senders: Default::default(),
        }
    }
}

impl<T: Message<Response = ()>> Actor for Broker<T> {}
impl<T: Message<Response = ()>> Service for Broker<T> {}

struct Publish<T: Message>(T);

impl<T: Message> Message for Publish<T> {
    type Response = ();
}

impl<T: Message<Response = ()> + Clone> Handler<Publish<T>> for Broker<T> {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Publish<T>) {
        self.subscribers
            .retain(|_, sender| sender.upgrade().is_some());

        let live_subscribers = self
            .subscribers
            .values()
            .filter_map(WeakSender::upgrade)
            .collect::<Vec<_>>();

        for subscriber in &live_subscribers {
            if let Err(_error) = subscriber.send(msg.0.clone()).await {
                // log::warn!("Failed to send message to subscriber: {:?}", error)
            }
        }

        for external_sender in &self.external_senders {
            if let Err(_error) = external_sender.send(msg.0.clone()).await {
                // log::warn!("Failed to send message to external sender: {:?}", error)
            }
        }

        self.external_senders.retain(|sender| !sender.is_closed());

        log::trace!(
            "published to {} subscribers, topic {:?}",
            live_subscribers.len(),
            std::any::type_name::<T>()
        );
    }
}

struct Subscribe<T: Message<Response = ()>>(WeakSender<T>);

impl<T: Message<Response = ()>> Message for Subscribe<T> {
    type Response = ();
}

struct SubscribeExternal<T: Message<Response = ()>>(async_channel::Sender<T>);

impl<T: Message<Response = ()>> Message for SubscribeExternal<T> {
    type Response = ();
}

struct Unsubscribe<T: Message<Response = ()>>(WeakSender<T>);

impl<T: Message<Response = ()>> Message for Unsubscribe<T> {
    type Response = ();
}

impl<T: Message<Response = ()> + Clone> Handler<Subscribe<T>> for Broker<T> {
    async fn handle(&mut self, _ctx: &mut Context<Self>, Subscribe(sender): Subscribe<T>) {
        self.subscribers.insert(sender.core.id, sender);
        log::trace!("subscribed to topic {:?}", std::any::type_name::<T>());
    }
}

impl<T: Message<Response = ()> + Clone> Handler<SubscribeExternal<T>> for Broker<T> {
    async fn handle(
        &mut self,
        _ctx: &mut Context<Self>,
        SubscribeExternal(sender): SubscribeExternal<T>,
    ) {
        self.external_senders.push(sender);
        log::trace!(
            "subscribed to external topic {:?}",
            std::any::type_name::<T>()
        );
    }
}

impl<T: Message<Response = ()> + Clone> Handler<Unsubscribe<T>> for Broker<T> {
    async fn handle(&mut self, _ctx: &mut Context<Self>, Unsubscribe(sender): Unsubscribe<T>) {
        self.subscribers.remove(&sender.core.id);
        log::trace!("unsubscribed to topic {:?}", std::any::type_name::<T>());
    }
}

impl<T: Message<Response = ()> + Clone> Addr<Broker<T>> {
    /// Publishes a message to all subscribers.
    pub async fn publish(&self, msg: T) -> crate::error::Result<()> {
        log::trace!("publishing to topic {:?}", std::any::type_name::<T>());
        self.send(Publish(msg)).await
    }

    /// Subscribes to messages of the given type.
    pub async fn subscribe(&self, sender: WeakSender<T>) -> crate::error::Result<()> {
        log::debug!("subscribing to topic {:?}", std::any::type_name::<T>());
        self.send(Subscribe(sender)).await
    }

    /// Unsubscribes from messages of the given type.
    pub async fn unsubscribe(&self, sender: WeakSender<T>) -> crate::error::Result<()> {
        self.send(Unsubscribe(sender)).await
    }
}

/// Subscribes to messages of the given type using a bounded channel.
pub async fn subscribe_to<T>(capacity: usize) -> crate::error::Result<async_channel::Receiver<T>>
where
    T: Message<Response = ()> + Clone,
{
    let (tx, rx) = async_channel::bounded(capacity);

    Broker::from_registry()
        .await
        .send(SubscribeExternal(tx))
        .await?;

    Ok(rx)
}

/// Subscribes to messages of the given type using an unbounded channel.
pub async fn subscribe_unbounded_to<T>() -> crate::error::Result<async_channel::Receiver<T>>
where
    T: Message<Response = ()> + Clone,
{
    let (tx, rx) = async_channel::unbounded();
    Broker::from_registry()
        .await
        .send(SubscribeExternal(tx))
        .await?;

    Ok(rx)
}

#[cfg(test)]
mod subscribe_publish_unsubscribe {
    #![allow(clippy::unwrap_used)]

    use futures::future::join;
    use hannibal_derive::Message;

    use crate::{Actor, Broker, Context, DynResult, Handler, Service, prelude::Spawnable as _};

    #[derive(Clone, Debug, Message)]
    struct Topic1(u32);

    #[derive(Default, Debug, PartialEq)]
    struct Subscribing(Vec<u32>);

    impl Actor for Subscribing {
        async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
            ctx.subscribe::<Topic1>().await?;
            Ok(())
        }
    }

    impl Handler<Topic1> for Subscribing {
        async fn handle(&mut self, ctx: &mut Context<Self>, msg: Topic1) {
            self.0.push(msg.0);
            if self.0.len() == 2 {
                ctx.stop().unwrap()
            }
        }
    }

    #[test_log::test(tokio::test)]
    async fn publish_different_ways() -> DynResult<()> {
        let mut subscriber1 = Subscribing::default().spawn_owning();
        subscriber1.ping().await.unwrap();

        let mut subscriber2 = Subscribing::default().spawn_owning();
        subscriber2.ping().await.unwrap();

        let ping_both = || join(subscriber1.ping(), subscriber2.ping());
        let _ = ping_both().await;

        let broker = Broker::from_registry().await;
        broker.publish(Topic1(42)).await.unwrap();
        broker.publish(Topic1(23)).await.unwrap();

        let _ = ping_both().await;

        assert_eq!(subscriber1.join().await, Some(Subscribing(vec![42, 23])));
        assert_eq!(subscriber2.join().await, Some(Subscribing(vec![42, 23])));

        Ok(())
    }

    #[test_log::test(tokio::test)]
    async fn external_subscriber_receives_published_messages() -> DynResult<()> {
        let rx = crate::subscribe_to::<Topic1>(10).await?;

        Broker::publish(Topic1(1)).await?;
        Broker::publish(Topic1(2)).await?;
        Broker::publish(Topic1(3)).await?;

        assert_eq!(rx.recv().await.unwrap().0, 1);
        assert_eq!(rx.recv().await.unwrap().0, 2);
        assert_eq!(rx.recv().await.unwrap().0, 3);

        Ok(())
    }
}
