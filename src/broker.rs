use std::collections::HashMap;

use crate::{context::ContextID, Actor, Addr, Context, Handler, Message, Service, WeakSender};

pub struct Broker<T: Message<Response = ()>> {
    subscribers: HashMap<ContextID, WeakSender<T>>,
}

impl<T: Message<Response = ()> + Clone> Broker<T> {
    pub async fn publish(topic: T) -> crate::error::Result<()> {
        Self::from_registry().await.publish(topic).await
    }

    pub async fn try_publish(topic: T) -> Option<crate::error::Result<()>> {
        // Self::try_from_registry().map(|broker| broker.publish(topic))
        if let Some(broker) = Self::try_from_registry() {
            Some(broker.publish(topic).await)
        } else {
            None
        }
    }

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
        for subscriber in self.subscribers.values().filter_map(WeakSender::upgrade) {
            if let Err(_error) = subscriber.send(msg.0.clone()).await {
                // log::warn!("Failed to send message to subscriber: {:?}", error)
            }
        }

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
        self.subscribers.insert(sender.id, sender);
    }
}

impl<T: Message<Response = ()> + Clone> Handler<Unsubscribe<T>> for Broker<T> {
    async fn handle(&mut self, _ctx: &mut Context<Self>, Unsubscribe(sender): Unsubscribe<T>) {
        self.subscribers.remove(&sender.id);
    }
}

impl<T: Message<Response = ()> + Clone> Addr<Broker<T>> {
    pub async fn publish(&self, msg: T) -> crate::error::Result<()> {
        self.send(Publish(msg)).await
    }

    pub async fn subscribe(&self, sender: WeakSender<T>) -> crate::error::Result<()> {
        self.send(Subscribe(sender)).await
    }

    pub async fn unsubscribe(&self, sender: WeakSender<T>) -> crate::error::Result<()> {
        self.send(Unsubscribe(sender)).await
    }
}

#[cfg(test)]
mod subscribe_publish_unsubscribe {
    #![allow(clippy::unwrap_used)]

    use futures::future::join;

    use crate::{
        prelude::Spawnable as _, Actor, Broker, Context, DynResult, Handler, Message, Service,
    };

    #[derive(Clone)]
    struct Topic1(u32);
    impl Message for Topic1 {
        type Response = ();
    }

    #[derive(Default, Debug, PartialEq)]
    struct Subscribing(Vec<u32>);

    impl Actor for Subscribing {
        async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
            ctx.subscribe::<Topic1>().await?;
            Ok(())
        }
    }

    impl Handler<Topic1> for Subscribing {
        async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Topic1) {
            self.0.push(msg.0);
        }
    }

    #[tokio::test]
    async fn publish_different_ways() -> DynResult<()> {
        let subscriber1 = Subscribing::default().spawn_owning();
        let subscriber2 = Subscribing::default().spawn_owning();

        let ping_both = || join(subscriber1.as_addr().ping(), subscriber2.as_addr().ping());
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
