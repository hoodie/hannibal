use futures::{FutureExt, channel::oneshot};
use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    task::Poll,
};
use weak_addr::WeakAddr;

pub mod caller;
pub mod sender;
pub mod weak_addr;
pub mod weak_caller;
pub mod weak_sender;

#[cfg(test)]
mod tests;

use crate::{
    RestartableActor,
    actor::{Actor, ActorHandle, JoinFuture},
    channel::Tx,
    context::Core,
    error::{ActorError, Result},
    event_loop::Payload,
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
    pub(crate) tx: Tx<A>,
    pub(crate) core: Core,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Addr {
            tx: self.tx.clone(),
            core: self.core.clone(),
        }
    }
}

impl<A: Actor> Addr<A> {
    /// Sends a stop signal to the actor.
    pub fn stop(&mut self) -> Result<()> {
        log::trace!("stopping actor");
        self.tx
            .force_send(Payload::Stop)
            .map_err(|_err| ActorError::AlreadyStopped)?;
        Ok(())
    }

    /// Halts the actor and awaits its termination.
    pub async fn halt(mut self) -> Result<()> {
        log::trace!("halting actor: stop and await");
        self.stop()?;
        self.await
    }

    /// Returns true if the actor is still running.
    pub fn running(&self) -> bool {
        self.core.running()
    }

    /// Returns true if the actor has stopped.
    pub fn stopped(&self) -> bool {
        self.core.stopped()
    }

    /// Sends a message to the actor and awaits its response.
    pub async fn call<M: Message>(&self, msg: M) -> Result<M::Response>
    where
        A: Handler<M>,
    {
        let (tx_response, response) = oneshot::channel();
        log::trace!("calling actor {}", std::any::type_name::<M>());
        self.tx
            .try_send(Payload::task(move |actor, ctx| {
                log::trace!("handling task call");
                Box::pin(async move {
                    log::trace!("actor handling call {}", std::any::type_name::<M>());
                    let res = Handler::handle(actor, ctx, msg).await;
                    let _ = tx_response.send(res);
                })
            }))
            .map_err(|_err| ActorError::AlreadyStopped)?;

        let response = response.await?;
        log::trace!("received response from actor");
        Ok(response)
    }

    /// Pings the actor to check if it is already/still alive.
    pub async fn ping(&self) -> Result<()> {
        log::trace!("pinging actor");
        let (tx_response, response) = oneshot::channel();
        self.tx
            .try_send(Payload::task(move |_actor, _ctx| {
                Box::pin(async move {
                    let _ = tx_response.send(());
                })
            }))
            .map_err(|_err| ActorError::AlreadyStopped)?;

        Ok(response.await?)
    }

    /// Sends a fire-and-forget message to the actor.
    pub async fn send<M: Message<Response = ()>>(&self, msg: M) -> Result<()>
    where
        A: Handler<M>,
    {
        log::trace!("sending message to actor {}", std::any::type_name::<M>());
        self.tx
            .send(Payload::task(move |actor, ctx| {
                Box::pin(Handler::handle(actor, ctx, msg))
            }))
            .await
            .map_err(|_err| ActorError::AlreadyStopped)?;
        Ok(())
    }

    /// Tries to send a fire-and-forget message to the actor.
    pub fn try_send<M: Message<Response = ()>>(&self, msg: M) -> Result<()>
    where
        A: Handler<M>,
    {
        log::trace!("sending message to actor {}", std::any::type_name::<M>());
        self.tx
            .try_send(Payload::task(move |actor, ctx| {
                Box::pin(Handler::handle(actor, ctx, msg))
            }))
            .map_err(|_err| ActorError::AlreadyStopped)?;
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
        self.tx
            .force_send(Payload::Restart)
            .map_err(|_err| ActorError::AlreadyStopped)?;
        Ok(())
    }
}

impl<A> Future for Addr<A> {
    type Output = Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        log::trace!("polling actor");
        self.get_mut().core.poll_unpin(cx)
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

    /// Give up ownership over the `Addr` and detach the underlying handle.
    ///
    /// # Drop behavior
    /// This is important when using the `async_runtime` feature where we hold on to the underlying [`task`](async_global_executor::Task).
    /// This `task` cancels itself when being dropped. This behavior is different when using `tokio_runtime`.
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
