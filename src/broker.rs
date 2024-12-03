use std::collections::HashMap;

use crate::{context::ContextID, Actor, Addr, Context, Handler, Message, Service, WeakSender};

pub struct Broker<T: Message<Result = ()>> {
    subscribers: HashMap<ContextID, WeakSender<T>>,
}

impl<T: Message<Result = ()> + Clone> Broker<T> {
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

impl<T: Message<Result = ()>> Default for Broker<T> {
    fn default() -> Self {
        Broker {
            subscribers: Default::default(),
        }
    }
}

impl<T: Message<Result = ()>> Actor for Broker<T> {}
impl<T: Message<Result = ()>> Service for Broker<T> {}

struct Publish<T: Message>(T);

impl<T: Message> Message for Publish<T> {
    type Result = ();
}

impl<T: Message<Result = ()> + Clone> Handler<Publish<T>> for Broker<T> {
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

struct Subscribe<T: Message<Result = ()>>(WeakSender<T>);

impl<T: Message<Result = ()>> Message for Subscribe<T> {
    type Result = ();
}

struct Unsubscribe<T: Message<Result = ()>>(WeakSender<T>);

impl<T: Message<Result = ()>> Message for Unsubscribe<T> {
    type Result = ();
}

impl<T: Message<Result = ()> + Clone> Handler<Subscribe<T>> for Broker<T> {
    async fn handle(&mut self, _ctx: &mut Context<Self>, Subscribe(sender): Subscribe<T>) {
        self.subscribers.insert(sender.id, sender);
    }
}

impl<T: Message<Result = ()> + Clone> Handler<Unsubscribe<T>> for Broker<T> {
    async fn handle(&mut self, _ctx: &mut Context<Self>, Unsubscribe(sender): Unsubscribe<T>) {
        self.subscribers.remove(&sender.id);
    }
}

impl<T: Message<Result = ()> + Clone> Addr<Broker<T>> {
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
    use std::time::Duration;

    use futures::future::join;
    use tokio::sync::oneshot;

    use crate::{
        prelude::Spawnable as _, Actor, Broker, Context, DynResult, Handler, Message, Service,
    };

    #[derive(Clone)]
    struct Topic1(u32);
    impl Message for Topic1 {
        type Result = ();
    }

    struct GetValue;
    impl Message for GetValue {
        type Result = Vec<u32>;
    }

    #[derive(Default)]
    struct Subscribing(Vec<u32>, Option<oneshot::Sender<()>>);

    impl Actor for Subscribing {
        async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
            ctx.subscribe::<Topic1>().await?;
            Ok(())
        }
    }

    impl Handler<Topic1> for Subscribing {
        async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Topic1) {
            self.0.push(msg.0);

            if let (Some(sender), true) = (self.1.take(), self.0.len() == 2) {
                let _ = sender.send(());
            }
        }
    }

    struct Notify(oneshot::Sender<()>);
    impl Message for Notify {
        type Result = ();
    }

    impl Handler<Notify> for Subscribing {
        async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Notify) {
            self.1.replace(msg.0);
        }
    }

    impl Handler<GetValue> for Subscribing {
        async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: GetValue) -> Vec<u32> {
            self.0.clone()
        }
    }

    #[tokio::test]
    async fn publish_different_ways() -> DynResult<()> {
        let subscriber1 = Subscribing::default().spawn();
        let subscriber2 = Subscribing::default().spawn();

        let (notify_tx1, notify_rx1) = oneshot::channel();
        let (notify_tx2, notify_rx2) = oneshot::channel();
        subscriber1.call(Notify(notify_tx1)).await.unwrap();
        subscriber2.call(Notify(notify_tx2)).await.unwrap();

        Broker::from_registry()
            .await
            .publish(Topic1(42))
            .await
            .unwrap();
        Broker::publish(Topic1(23)).await.unwrap();
        let _ = join(notify_rx1, notify_rx2).await;

        tokio::time::sleep(Duration::from_secs(1)).await; // Wait for the messages

        assert_eq!(subscriber1.call(GetValue).await?, vec![42, 23]);
        assert_eq!(subscriber2.call(GetValue).await?, vec![42, 23]);

        Ok(())
    }
}
