use dyn_clone::DynClone;
use futures::channel::oneshot;

use std::{future::Future, pin::Pin};

use crate::{Actor, Handler, channel, context::Core, error::ActorError};

use super::{Addr, Message, Payload, Result, weak_caller::WeakCaller};

/// A strong reference to some actor that can receive a message `M` and respond.
///
/// Can be used to send a message to an actor and receive a response.
/// If you don't need a response, use [`Sender`](`crate::Sender`) instead.
///
/// Callers can be downgraded to [`WeakCaller`](`crate::WeakCaller`) to check if the actor is still alive.
pub struct Caller<M: Message> {
    core: Core,
    call_fn: Box<dyn CallerFn<M>>,
    downgrade_fn: Box<dyn DowngradeFn<M>>,
}

impl<M: Message> Caller<M> {
    /// Sends a message to the actor and awaits its response.
    pub async fn call(&self, msg: M) -> Result<M::Response> {
        self.call_fn.call(msg).await
    }

    /// Downgrades this to a weak callers that does not keep the actor alive.
    pub fn downgrade(&self) -> WeakCaller<M> {
        self.downgrade_fn.downgrade()
    }

    pub(crate) fn new<A>(tx: channel::Tx<A>, core: Core) -> Self
    where
        A: Actor + Handler<M>,
    {
        let weak_tx = tx.downgrade();

        let call_fn = {
            let core = core.clone();
            Box::new(
                move |msg| -> Pin<Box<dyn Future<Output = Result<M::Response>>>> {
                    let tx = tx.clone();
                    let core = core.clone();
                    Box::pin(async move {
                        if core.stopped() {
                            return Err(ActorError::ChannelClosed);
                        }

                        let (response_tx, response) = oneshot::channel();

                        tx.send(Payload::task(move |actor, ctx| {
                            Box::pin(async move {
                                let res = Handler::handle(&mut *actor, ctx, msg).await;
                                let _ = response_tx.send(res);
                            })
                        }))
                        .await
                        .map_err(|_err| ActorError::ChannelClosed)?;

                        Ok(response.await?)
                    })
                },
            )
        };

        let upgrade = {
            let core = core.clone();
            Box::new(move || weak_tx.upgrade().map(|tx| Caller::new(tx, core.clone())))
        };

        let downgrade_fn = {
            let core = core.clone();
            Box::new(move || WeakCaller {
                upgrade: upgrade.clone(),
                core: core.clone(),
            })
        };

        Caller {
            core,
            call_fn,
            downgrade_fn,
        }
    }
}

impl<M: Message> Caller<M> {
    /// Returns true if the actor is still running.
    pub fn running(&self) -> bool {
        self.core.running()
    }

    /// Returns true if the actor has stopped.
    pub fn stopped(&self) -> bool {
        self.core.stopped()
    }
}

trait CallerFn<M: Message>: Send + Sync + 'static + DynClone {
    fn call(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<M::Response>>>>;
}

impl<F, M> CallerFn<M> for F
where
    F: Fn(M) -> Pin<Box<dyn Future<Output = Result<M::Response>>>>,
    F: 'static + Send + Sync + Clone,
    M: Message,
{
    fn call(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<M::Response>>>> {
        self(msg)
    }
}

impl<M: Message, A> From<Addr<A>> for Caller<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        Caller::new(addr.tx.clone(), addr.core)
    }
}

impl<M: Message> Clone for Caller<M> {
    fn clone(&self) -> Self {
        Caller {
            core: self.core.clone(),
            call_fn: dyn_clone::clone_box(&*self.call_fn),
            downgrade_fn: dyn_clone::clone_box(&*self.downgrade_fn),
        }
    }
}

trait DowngradeFn<M: Message>: Send + Sync + 'static + DynClone {
    fn downgrade(&self) -> WeakCaller<M>;
}

impl<F, M> DowngradeFn<M> for F
where
    F: Fn() -> WeakCaller<M>,
    F: 'static + Send + Sync + DynClone,
    M: Message,
{
    fn downgrade(&self) -> WeakCaller<M> {
        self()
    }
}
