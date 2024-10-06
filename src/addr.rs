use futures::{channel::oneshot, FutureExt};
use std::{future::Future, pin::Pin, sync::Arc, task::Poll};
use weak_addr::WeakAddr;

pub mod caller;
pub mod sender;
pub mod weak_addr;
pub mod weak_caller;
pub mod weak_sender;

use crate::{
    actor::{Actor, Handler},
    channel::ChanTx,
    context::{Context, RunningFuture},
    error::Result,
};

type TaskFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

type TaskFn<A> =
    Box<dyn for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> TaskFuture<'a> + Send + 'static>;

pub(crate) enum Payload<A> {
    Task(TaskFn<A>),
    Stop,
}

impl<A: Actor> Payload<A> {
    pub fn task<F>(f: F) -> Self
    where
        F: for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> TaskFuture<'a> + Send + 'static,
    {
        Self::Task(Box::new(f))
    }
}

pub trait Message: 'static + Send {
    type Result: 'static + Send;
}

pub struct Addr<A> {
    pub(crate) payload_tx: ChanTx<A>,
    pub(crate) running: RunningFuture,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Addr {
            payload_tx: Arc::clone(&self.payload_tx),
            running: self.running.clone(),
        }
    }
}

impl<A: Actor> Addr<A> {
    pub fn stop(&mut self) -> Result<()> {
        self.payload_tx.send(Payload::Stop)?;
        Ok(())
    }

    pub fn stopped(&self) -> bool {
        self.running.peek().is_some()
    }

    pub async fn call<M: Message>(&self, msg: M) -> Result<M::Result>
    where
        A: Handler<M>,
    {
        let (tx_response, response) = oneshot::channel();
        self.payload_tx.send(Payload::task(move |actor, ctx| {
            Box::pin(async move {
                let res = Handler::handle(actor, ctx, msg).await;
                let _ = tx_response.send(res);
            })
        }))?;

        Ok(response.await?)
    }

    pub fn send<M: Message<Result = ()>>(&self, msg: M) -> Result<()>
    where
        A: Handler<M>,
    {
        self.payload_tx.send(Payload::task(move |actor, ctx| {
            Box::pin(Handler::handle(actor, ctx, msg))
        }))?;
        Ok(())
    }

    pub fn downgrade(&self) -> WeakAddr<A> {
        WeakAddr::from(self)
    }
}

impl<A> Future for Addr<A> {
    type Output = Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        self.get_mut()
            .running
            .poll_unpin(cx)
            .map(|p| p.map_err(Into::into))
    }
}

#[cfg(test)]
mod tests {
    use crate::{ActorResult, Environment};

    use super::*;
    use std::future::Future;

    #[derive(Debug, Default)]
    pub struct MyActor(pub Option<&'static str>);

    pub struct Stop;
    impl Message for Stop {
        type Result = ();
    }

    pub struct Store(pub &'static str);
    impl Message for Store {
        type Result = ();
    }

    pub struct Add(pub i32, pub i32);
    impl Message for Add {
        type Result = i32;
    }

    impl Actor for MyActor {}

    impl Handler<Stop> for MyActor {
        async fn handle(&mut self, ctx: &mut Context<Self>, _: Stop) {
            if let Err(e) = ctx.stop() {
                eprintln!("{}", e);
            }
        }
    }

    impl Handler<Store> for MyActor {
        async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Store) {
            self.0.replace(msg.0);
        }
    }

    impl Handler<Add> for MyActor {
        async fn handle(&mut self, _: &mut Context<Self>, msg: Add) -> i32 {
            msg.0 + msg.1
        }
    }

    pub fn start<A: Actor>(actor: A) -> (impl Future<Output = ActorResult<A>>, Addr<A>) {
        let (event_loop, addr) = Environment::unbounded().launch(actor);
        (event_loop, addr)
    }

    #[tokio::test]
    async fn addr_call() {
        let (event_loop, addr) = start(MyActor::default());
        tokio::spawn(event_loop);

        let addition = addr.call(Add(1, 2)).await.unwrap();
        assert_eq!(addition, 3);
    }

    #[tokio::test]
    async fn addr_send() {
        let (event_loop, mut addr) = start(MyActor::default());
        let task = tokio::spawn(event_loop);
        addr.send(Store("password")).unwrap();
        addr.stop().unwrap();
        let actor = task.await.unwrap().unwrap();
        assert_eq!(actor.0, Some("password"))
    }

    #[tokio::test]
    async fn addr_send_err() {
        let (event_loop, mut addr) = start(MyActor::default());
        tokio::spawn(event_loop);
        let addr2 = addr.clone();
        addr.stop().unwrap();
        addr.await.unwrap();
        assert!(addr2.send(Store("password")).is_err());
    }

    #[tokio::test]
    async fn addr_stop() {
        let (event_loop, mut addr) = start(MyActor::default());
        tokio::spawn(event_loop);

        let addr2 = addr.clone();
        addr.stop().unwrap();

        addr2.await.unwrap();
        addr.await.unwrap();
    }

    #[tokio::test]
    async fn ctx_stop() {
        let (event_loop, addr) = start(MyActor::default());
        tokio::spawn(event_loop);

        let addr2 = addr.clone();
        addr.send(Stop).unwrap();

        addr2.await.unwrap();
        addr.await.unwrap();
    }

    #[tokio::test]
    async fn addr_stopped_after_stop() {
        let (event_loop, addr) = start(MyActor::default());
        tokio::spawn(event_loop);

        let addr2 = addr.clone();
        assert!(!addr2.stopped(), "addr2 should not be stopped");

        addr.send(Stop).unwrap();

        addr.await.unwrap();
        assert!(addr2.stopped(), "addr2 should be stopped");
    }

    #[tokio::test]
    async fn weak_addr_does_not_prolong_life() {
        let (event_loop, addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_addr = WeakAddr::from(&addr);
        weak_addr.upgrade().unwrap();

        drop(addr);

        assert!(weak_addr.upgrade().is_none());
        actor.await.unwrap().unwrap();
    }
}
