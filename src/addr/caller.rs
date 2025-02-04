use dyn_clone::DynClone;
use futures::channel::oneshot;

use std::sync::{Arc, Weak};
use std::{future::Future, pin::Pin};

use crate::{channel::ChanTx, context::ContextID, Actor, Handler};

use super::{weak_caller::WeakCaller, Addr, Message, Payload, Result};

/// A reference to some actor that can receive `M`.
///
/// Can be used to send a message to an actor and receive a response.
/// If you don't need a response, use [`Sender`](`crate::Sender`) instead.
///
/// Callers can be downgraded to [`WeakCaller`](`crate::WeakCaller`) to check if the actor is still alive.
pub struct Caller<M: Message> {
    call_fn: Box<dyn CallerFn<M>>,
    downgrade_fn: Box<dyn DowngradeFn<M>>,
    id: ContextID,
}

impl<M: Message> Caller<M> {
    pub async fn call(&self, msg: M) -> Result<M::Response> {
        self.call_fn.call(msg).await
    }

    pub fn downgrade(&self) -> WeakCaller<M> {
        self.downgrade_fn.downgrade()
    }

    pub(crate) fn new<A>(tx: ChanTx<A>, id: ContextID) -> Self
    where
        A: Actor + Handler<M>,
    {
        let weak_tx: Weak<_> = Arc::downgrade(&tx);

        // TODO: make this queue-safe
        let call_fn = Box::new(
            move |msg| -> Pin<Box<dyn Future<Output = Result<M::Response>>>> {
                let tx = Arc::clone(&tx);
                Box::pin(async move {
                    let (response_tx, response) = oneshot::channel();

                    // TODO: make this queue-safe
                    tx.send(Payload::task(move |actor, ctx| {
                        Box::pin(async move {
                            let res = Handler::handle(&mut *actor, ctx, msg).await;
                            let _ = response_tx.send(res);
                        })
                    }))
                    .await?;

                    Ok(response.await?)
                })
            },
        );

        let upgrade = Box::new(move || weak_tx.upgrade().map(|tx| Caller::new(tx, id)));

        let downgrade_fn = Box::new(move || WeakCaller {
            upgrade: upgrade.clone(),
            id,
        });

        Caller {
            id,
            call_fn,
            downgrade_fn,
        }
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
        Caller::new(addr.payload_tx.to_owned(), addr.context_id)
    }
}

impl<M: Message> Clone for Caller<M> {
    fn clone(&self) -> Self {
        Caller {
            id: self.id,
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
