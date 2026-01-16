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
        log::trace!(
            "published to {} subscribers, topic {:?}",
            live_subscribers.len(),
            std::any::type_name::<T>()
        );

        self.subscribers
            .retain(|_, sender| sender.upgrade().is_some());
    }
}

struct Subscribe<T: Message<Response = ()>>(WeakSender<T>);

impl<T: Message<Response = ()>> Message for Subscribe<T> {
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
        let subscriber1 = Subscribing::default().spawn_owning();
        subscriber1.ping().await.unwrap();

        let subscriber2 = Subscribing::default().spawn_owning();
        subscriber2.ping().await.unwrap();

        let ping_both = || join(subscriber1.ping(), subscriber2.ping());
        let _ = ping_both().await;

        let broker = Broker::from_registry().await;
        broker.publish(Topic1(42)).await.unwrap();
        broker.publish(Topic1(23)).await.unwrap();

        let _ = ping_both().await;

        assert_eq!(subscriber1.consume().await, Ok(Subscribing(vec![42, 23])));
        assert_eq!(subscriber2.consume().await, Ok(Subscribing(vec![42, 23])));

        Ok(())
    }
}
