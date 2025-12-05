use dyn_clone::DynClone;

use std::{future::Future, pin::Pin};

use crate::{Actor, Handler, channel, context::Core, error::ActorError};

use super::{Addr, Message, Payload, Result, weak_sender::WeakSender};

/// A strong reference to some actor that can receive message `M`.
///
/// Can be used to send a message to an actor without expecting a response.
/// If you need a response, use [`Caller`](`crate::Caller`) instead.
///
/// Senders can be downgraded to [`WeakSender`](`crate::WeakSender`) to check if the actor is still alive.
pub struct Sender<M: Message<Response = ()>> {
    core: Core,
    send_fn: Box<dyn SenderFn<M>>,
    try_send_fn: Box<dyn TrySenderFn<M>>,
    force_send_fn: Box<dyn ForceSenderFn<M>>,
    downgrade_fn: Box<dyn DowngradeFn<M>>,
}

impl<M: Message<Response = ()>> Sender<M> {
    /// Sends a message to the actor.
    pub fn send(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        self.send_fn.send(msg)
    }

    pub(crate) fn force_send(&self, msg: M) -> Result<()> {
        self.force_send_fn.send(msg)
    }

    /// Tries to send a fire-and-forget message to the actor.
    pub fn try_send(&self, msg: M) -> Result<()> {
        self.try_send_fn.try_send(msg)
    }

    /// Downgrades this to a weak senders that does not keep the actor alive.
    pub fn downgrade(&self) -> WeakSender<M> {
        self.downgrade_fn.downgrade()
    }

    pub(crate) fn new<A>(tx: channel::Tx<A>, core: Core) -> Self
    where
        A: Actor + Handler<M>,
    {
        let weak_tx = tx.downgrade();

        let send_fn: Box<dyn SenderFn<M>> = {
            let tx = tx.clone();
            Box::new(
                move |msg: M| -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
                    let tx = tx.clone();
                    Box::pin(async move {
                        tx.send(Payload::task(move |actor, ctx| {
                            Box::pin(Handler::handle(&mut *actor, ctx, msg))
                        }))
                        .await
                        .map_err(|_err| ActorError::AlreadyStopped)?;
                        Ok(())
                    })
                },
            )
        };

        let try_send_fn: Box<dyn TrySenderFn<M>> = {
            let tx = tx.clone();
            Box::new(move |msg: M| {
                tx.try_send(Payload::task(move |actor, ctx| {
                    Box::pin(Handler::handle(&mut *actor, ctx, msg))
                }))
                .map_err(|_err| ActorError::AlreadyStopped)?;
                Ok(())
            })
        };

        let force_send_fn: Box<dyn ForceSenderFn<M>> = {
            Box::new(move |msg| {
                tx.force_send(Payload::task(move |actor, ctx| {
                    Box::pin(Handler::handle(&mut *actor, ctx, msg))
                }))
                .map_err(|_err| ActorError::AlreadyStopped)?;
                Ok(())
            })
        };

        let downgrade_fn: Box<dyn DowngradeFn<M>> = {
            let core = core.clone();
            Box::new(move || {
                let weak_tx = weak_tx.clone();
                let core = core.clone();
                WeakSender {
                    core: core.clone(),
                    upgrade: Box::new(move || {
                        weak_tx.upgrade().map(|tx| Sender::new(tx, core.clone()))
                    }),
                }
            })
        };

        Sender {
            core,
            send_fn,
            try_send_fn,
            force_send_fn,
            downgrade_fn,
        }
    }
}

impl<M: Message<Response = ()>> Sender<M> {
    /// Returns true if the actor is still running.
    pub fn running(&self) -> bool {
        self.core.running()
    }

    /// Returns true if the actor has stopped.
    pub fn stopped(&self) -> bool {
        self.core.stopped()
    }
}

trait ForceSenderFn<M: Message<Response = ()>>: 'static + Send + Sync + DynClone {
    fn send(&self, msg: M) -> Result<()>;
}

impl<F, M: Message<Response = ()>> ForceSenderFn<M> for F
where
    F: Fn(M) -> Result<()>,
    F: 'static + Send + Sync + Clone,
{
    fn send(&self, msg: M) -> Result<()> {
        self(msg)
    }
}

trait SenderFn<M: Message<Response = ()>>: 'static + Send + Sync + DynClone {
    fn send(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;
}

impl<F, M: Message<Response = ()>> SenderFn<M> for F
where
    F: Fn(M) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>,
    F: 'static + Send + Sync + Clone,
{
    fn send(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        self(msg)
    }
}

trait TrySenderFn<M: Message<Response = ()>>: 'static + Send + Sync + DynClone {
    fn try_send(&self, msg: M) -> Result<()>;
}

impl<F, M: Message<Response = ()>> TrySenderFn<M> for F
where
    F: Fn(M) -> Result<()>,
    F: 'static + Send + Sync + Clone,
{
    fn try_send(&self, msg: M) -> Result<()> {
        self(msg)
    }
}

impl<M: Message<Response = ()>, A> From<Addr<A>> for Sender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        Sender::new(addr.tx.clone(), addr.core)
    }
}

impl<M: Message<Response = ()>, A> From<&Addr<A>> for Sender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: &Addr<A>) -> Self {
        Sender::new(addr.tx.clone(), addr.core.clone())
    }
}

impl<M: Message<Response = ()>> Clone for Sender<M> {
    fn clone(&self) -> Self {
        Sender {
            core: self.core.clone(),
            send_fn: dyn_clone::clone_box(&*self.send_fn),
            try_send_fn: dyn_clone::clone_box(&*self.try_send_fn),
            force_send_fn: dyn_clone::clone_box(&*self.force_send_fn),
            downgrade_fn: dyn_clone::clone_box(&*self.downgrade_fn),
        }
    }
}

trait DowngradeFn<M: Message<Response = ()>>: Send + Sync + 'static + DynClone {
    fn downgrade(&self) -> WeakSender<M>;
}

impl<F, M> DowngradeFn<M> for F
where
    F: Fn() -> WeakSender<M>,
    F: 'static + Send + Sync + Clone,
    M: Message<Response = ()>,
{
    fn downgrade(&self) -> WeakSender<M> {
        self()
    }
}
