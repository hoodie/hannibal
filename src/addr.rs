use futures::{FutureExt, channel::oneshot};
use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::Arc,
    task::Poll,
};
use weak_addr::WeakAddr;

pub mod caller;
pub mod sender;
pub mod weak_addr;
pub mod weak_caller;
pub mod weak_sender;

use crate::{
    RestartableActor,
    actor::{Actor, ActorHandle, JoinFuture},
    channel::{ChanTx, ForceChanTx},
    context::{ContextID, RunningFuture},
    environment::Payload,
    error::Result,
    handler::Handler,
};

/// Anything that you want to send to an actor.
///
/// Messages can have a response type like `Ping` and `Pong` in the following example.
/// ```rust
/// # use hannibal::message;
/// #[message(response = Pong)]
/// struct Ping;
///
/// struct Pong; // does not need to implement `Message`
/// ```
///
/// Or they can be fire-and-forget messages like `Stop` in the following example.
///
/// ```rust
/// # use hannibal::message;
/// #[message]
/// struct Store(&'static str);
/// ```
///
/// You can also derive the `Message` trait for simple messages without a response.
///
/// ```rust
/// # use hannibal::prelude::*;
/// #[derive(Debug, Message)]
/// struct Store(&'static str);
/// ```
///
pub trait Message: 'static + Send {
    /// What the actor should respond with.
    type Response: 'static + Send;
}

impl Message for () {
    type Response = ();
}

/// A strong reference to an actor.
///
/// This is the main way to interact with an actor.
/// You get the `Addr` when you spawn an actor.
/// `Addr`s can be cloned and sent to other threads.
///
/// # Weak References
/// The [`Actor`] will be stopped when the last [`Addr`]s are dropped,
/// if you don't want to prolong the life of the actor you can use a [`WeakAddr`] via [`Addr::downgrade`].
///
pub struct Addr<A> {
    pub(crate) context_id: ContextID,
    pub(crate) payload_tx: ChanTx<A>,
    pub(crate) payload_force_tx: ForceChanTx<A>,
    pub(crate) running: RunningFuture,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Addr {
            context_id: self.context_id,
            payload_tx: Arc::clone(&self.payload_tx),
            payload_force_tx: Arc::clone(&self.payload_force_tx),
            running: self.running.clone(),
        }
    }
}

impl<A: Actor> Addr<A> {
    /// Sends a stop signal to the actor.
    pub fn stop(&mut self) -> Result<()> {
        log::trace!("stopping actor");
        self.payload_force_tx.send(Payload::Stop)?;
        Ok(())
    }

    /// Halts the actor and awaits its termination.
    pub async fn halt(mut self) -> Result<()> {
        log::trace!("halting actor");
        self.stop()?;
        self.await
    }

    /// Returns true if the actor is still running.
    pub fn running(&self) -> bool {
        self.running.peek().is_none()
    }

    /// Returns true if the actor has stopped.
    pub fn stopped(&self) -> bool {
        self.running.peek().is_some()
    }

    /// Sends a message to the actor and awaits its response.
    pub async fn call<M: Message>(&self, msg: M) -> Result<M::Response>
    where
        A: Handler<M>,
    {
        let (tx_response, response) = oneshot::channel();
        log::trace!("calling actor {}", std::any::type_name::<M>());
        self.payload_force_tx
            .send(Payload::task(move |actor, ctx| {
                log::trace!("handling task call");
                Box::pin(async move {
                    log::trace!("actor handling call {}", std::any::type_name::<M>());
                    let res = Handler::handle(actor, ctx, msg).await;
                    let _ = tx_response.send(res);
                })
            }))?;

        let response = response.await?;
        log::trace!("received response from actor");
        Ok(response)
    }

    /// Pings the actor to check if it is already/still alive.
    pub async fn ping(&self) -> Result<()> {
        log::trace!("pinging actor");
        let (tx_response, response) = oneshot::channel();
        self.payload_force_tx
            .send(Payload::task(move |_actor, _ctx| {
                Box::pin(async move {
                    let _ = tx_response.send(());
                })
            }))?;

        Ok(response.await?)
    }

    // TODO: look if this can be made available exclusively to unbounded environments
    #[allow(dead_code)]
    pub(crate) fn force_send<M: Message<Response = ()>>(&self, msg: M) -> Result<()>
    where
        A: Handler<M>,
    {
        log::trace!(
            "force sending message to actor {}",
            std::any::type_name::<M>()
        );
        self.payload_force_tx
            .send(Payload::task(move |actor, ctx| {
                Box::pin(Handler::handle(actor, ctx, msg))
            }))?;
        Ok(())
    }

    /// Sends a fire-and-forget message to the actor.
    pub async fn send<M: Message<Response = ()>>(&self, msg: M) -> Result<()>
    where
        A: Handler<M>,
    {
        log::trace!("sending message to actor {}", std::any::type_name::<M>());
        self.payload_tx
            .send(Payload::task(move |actor, ctx| {
                Box::pin(Handler::handle(actor, ctx, msg))
            }))
            .await?;
        Ok(())
    }

    /// Downgrades this address to a weak address that does not keep the actor alive.
    pub fn downgrade(&self) -> WeakAddr<A> {
        WeakAddr::from(self)
    }

    /// Creates a sender for messages to the actor.
    pub fn sender<M: Message<Response = ()>>(&self) -> sender::Sender<M>
    where
        A: Handler<M>,
    {
        log::trace!("creating sender for actor {}", std::any::type_name::<M>());
        sender::Sender::from(self.to_owned())
    }

    /// Creates a weak sender for messages to the actor.
    pub fn weak_sender<M: Message<Response = ()>>(&self) -> weak_sender::WeakSender<M>
    where
        A: Handler<M>,
    {
        weak_sender::WeakSender::from(self.to_owned())
    }

    /// Creates a caller for sending messages that expect a response.
    pub fn caller<M: Message>(&self) -> caller::Caller<M>
    where
        A: Handler<M>,
    {
        caller::Caller::from(self.to_owned())
    }

    /// Creates a weak caller for sending messages that expect a response.
    pub fn weak_caller<M: Message>(&self) -> weak_caller::WeakCaller<M>
    where
        A: Handler<M>,
    {
        weak_caller::WeakCaller::from(self.to_owned())
    }
}

impl<A: RestartableActor> Addr<A> {
    /// Restart the actor. This is not possible for all actors.
    ///
    /// [`StreamHandlers`](`crate::StreamHandler`) for example can't be restarted.
    pub fn restart(&mut self) -> Result<()> {
        self.payload_force_tx.send(Payload::Restart)?;
        Ok(())
    }
}

impl<A> Future for Addr<A> {
    type Output = Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        log::trace!("polling actor");
        self.get_mut()
            .running
            .poll_unpin(cx)
            .map(|p| p.map_err(Into::into))
    }
}

/// An even stronger reference to an actor.
///
/// Owning Addresses can be used to retrieve the actor after it has been stopped.
/// There can only be one owning Address to an instance of an actor.
/// They can be downgraded to normal [`Addr`]s.
pub struct OwningAddr<A> {
    pub(crate) addr: Addr<A>,
    pub(crate) handle: ActorHandle<A>,
}

impl<A: Actor> OwningAddr<A> {
    pub(crate) const fn new(addr: Addr<A>, handle: ActorHandle<A>) -> Self {
        OwningAddr { addr, handle }
    }

    /// Waits for the actor to stop and returns it.
    pub fn join(&mut self) -> JoinFuture<A> {
        log::trace!("joining actor");
        self.handle.join()
    }

    /// Stops the actor and returns it.
    pub async fn consume(mut self) -> Result<A> {
        log::trace!("consuming actor");
        self.addr.stop()?;
        self.join()
            .await
            .ok_or(crate::error::ActorError::AlreadyStopped)
    }

    /// Stops the actor and returns it.
    ///
    /// In contrast to `halt()` if stop fails you will get an error before waiting for the actor to stop.
    /// That does not mean that the join itself isn't still fallible.
    pub fn consume_sync(mut self) -> Result<JoinFuture<A>> {
        log::trace!("consuming actor synchronously");
        self.addr.stop()?;
        Ok(self.join())
    }

    pub fn detach(self) -> Addr<A> {
        log::trace!("detaching owning addr");
        self.handle.detach();
        self.addr
    }
}

impl<A> AsRef<Addr<A>> for OwningAddr<A> {
    fn as_ref(&self) -> &Addr<A> {
        &self.addr
    }
}

impl<A> Deref for OwningAddr<A> {
    type Target = Addr<A>;

    fn deref(&self) -> &Self::Target {
        &self.addr
    }
}

impl<A> DerefMut for OwningAddr<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.addr
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::{Context, DynResult, Message, environment::Environment};

    use super::*;
    use std::future::Future;

    #[derive(Debug, Default)]
    pub struct MyActor(pub Option<&'static str>);

    pub struct Stop;
    impl Message for Stop {
        type Response = ();
    }

    pub struct Store(pub &'static str);
    impl Message for Store {
        type Response = ();
    }

    pub struct Add(pub i32, pub i32);
    impl Message for Add {
        type Response = i32;
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

    pub fn start<A: Actor>(actor: A) -> (impl Future<Output = DynResult<A>>, Addr<A>) {
        let (event_loop, addr) = Environment::unbounded().create_loop(actor);
        (event_loop, addr)
    }

    #[test_log::test(tokio::test)]
    async fn addr_call() {
        let (event_loop, addr) = start(MyActor::default());
        tokio::spawn(event_loop);

        let addition = addr.call(Add(1, 2)).await.unwrap();
        assert_eq!(addition, 3);
    }

    #[test_log::test(tokio::test)]
    async fn addr_send() {
        let (event_loop, mut addr) = start(MyActor::default());
        let task = tokio::spawn(event_loop);
        addr.send(Store("password")).await.unwrap();
        addr.stop().unwrap();
        let actor = task.await.unwrap().unwrap();
        assert_eq!(actor.0, Some("password"))
    }

    #[test_log::test(tokio::test)]
    async fn addr_send_err() {
        let (event_loop, mut addr) = start(MyActor::default());
        tokio::spawn(event_loop);
        let addr2 = addr.clone();
        addr.stop().unwrap();
        addr.await.unwrap();
        assert!(addr2.send(Store("password")).await.is_err());
    }

    #[test_log::test(tokio::test)]
    async fn addr_stop() {
        let (event_loop, mut addr) = start(MyActor::default());
        tokio::spawn(event_loop);

        let addr2 = addr.clone();
        addr.stop().unwrap();

        addr2.await.unwrap();
        addr.await.unwrap();
    }

    #[test_log::test(tokio::test)]
    async fn ctx_stop() {
        let (event_loop, addr) = start(MyActor::default());
        tokio::spawn(event_loop);

        let addr2 = addr.clone();
        addr.send(Stop).await.unwrap();

        addr2.await.unwrap();
        addr.await.unwrap();
    }

    #[test_log::test(tokio::test)]
    async fn addr_stopped_after_stop() {
        let (event_loop, addr) = start(MyActor::default());
        tokio::spawn(event_loop);

        let addr2 = addr.clone();
        assert!(!addr2.stopped(), "addr2 should not be stopped");

        addr.send(Stop).await.unwrap();

        addr.await.unwrap();
        assert!(addr2.stopped(), "addr2 should be stopped");
    }

    #[test_log::test(tokio::test)]
    async fn weak_addr_does_not_prolong_life() {
        let (event_loop, addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_addr = WeakAddr::from(&addr);
        let weak_addr2 = addr.downgrade();
        weak_addr.upgrade().unwrap();
        weak_addr2.upgrade().unwrap();

        drop(addr);

        assert!(weak_addr.upgrade().is_none());
        assert!(weak_addr2.upgrade().is_none());
        actor.await.unwrap().unwrap();
    }

    #[test_log::test(tokio::test)]
    async fn weak_caller_does_not_prolong_life() {
        let (event_loop, addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_caller = addr.weak_caller::<Stop>();
        weak_caller.upgrade().unwrap();
        let weak_caller2 = addr.caller::<Stop>().downgrade();
        weak_caller2.upgrade().unwrap();

        drop(addr);

        assert!(weak_caller.upgrade().is_none());
        assert!(weak_caller2.upgrade().is_none());
        actor.await.unwrap().unwrap();
    }

    #[test_log::test(tokio::test)]
    async fn weak_sender_does_not_prolong_life() {
        let (event_loop, addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_sender = addr.weak_sender::<Stop>();
        weak_sender.upgrade().unwrap();
        let weak_sender2 = addr.sender::<Stop>().downgrade();
        weak_sender2.upgrade().unwrap();

        drop(addr);

        assert!(weak_sender.upgrade().is_none());
        assert!(weak_sender2.upgrade().is_none());
        actor.await.unwrap().unwrap();
    }
}
