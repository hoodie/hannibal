use dyn_clone::DynClone;
use futures::channel::oneshot;

use std::sync::{Arc, Weak};
use std::{future::Future, pin::Pin};

use crate::{channel::ChanTx, Actor, ActorId, Handler};

use super::{weak_caller::WeakCaller, ActorEvent, Addr, Message, Result};

trait CallerFn<M: Message>: Send + Sync + 'static + DynClone {
    fn call(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>>;
}

impl<F, M> CallerFn<M> for F
where
    F: Fn(M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>>,
    F: 'static + Send + Sync + Clone,
    M: Message,
{
    fn call(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>> {
        self(msg)
    }
}

/// Lets you call an actor with a message and wait for the result.
///
/// A `Caller` allows you to wait for the result of a `Handler`, unlike [`super::Sender`] which is for messages with `Result = ()`.
/// This struct is created via [`Addr::caller()`](`super::Addr::caller`).
///
/// Like [`Addr<A>`](`super::Addr<A>`), `Caller` has a *strong* reference to the recipient of the message type,
/// and so will prevent an actor from stopping if all [`crate::Addr`]'s have been dropped elsewhere.
///
/// You can downgrade a `Caller` to a [`WeakCaller`] to avoid creating a reference cycle.
pub struct Caller<M: Message> {
    actor_id: ActorId,
    call_fn: Box<dyn CallerFn<M>>,
    downgrade_fn: Box<dyn DowngradeFn<M>>,
}

impl<M: Message> Caller<M> {
    /// Call the actor with the given message and wait for the result.
    pub async fn call(&self, msg: M) -> Result<M::Result> {
        self.call_fn.call(msg).await
    }

    /// Downgrade to a [`WeakCaller`] to avoid creating a reference cycle.
    pub fn downgrade(&self) -> WeakCaller<M> {
        self.downgrade_fn.downgrade()
    }
}

impl<M: Message> PartialEq for Caller<M> {
    fn eq(&self, other: &Self) -> bool {
        self.actor_id.eq(&other.actor_id)
    }
}

impl<M: Message, A> From<Addr<A>> for Caller<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        addr.pieces().into()
    }
}

impl<M: Message, A> From<(ActorId, ChanTx<A>)> for Caller<M>
where
    A: Actor + Handler<M>,
{
    fn from((actor_id, tx): (ActorId, ChanTx<A>)) -> Self {
        let weak_tx: Weak<_> = Arc::downgrade(&tx);

        let call_fn = Box::new(move |msg| {
            let tx: Arc<_> = tx.clone();
            Box::pin(async move {
                let (response_tx, response) = oneshot::channel();

                tx.send(ActorEvent::exec(move |actor, ctx| {
                    Box::pin(async move {
                        let res = Handler::handle(&mut *actor, ctx, msg).await;
                        let _ = response_tx.send(res);
                    })
                }))?;

                Ok(response.await?)
            }) as Pin<Box<dyn Future<Output = Result<M::Result>>>>
        });

        let upgrade = Box::new(move || weak_tx.upgrade().map(|tx| Caller::from((actor_id, tx))));
        let downgrade_fn = Box::new(move || WeakCaller {
            actor_id,
            upgrade: upgrade.clone(),
        });

        Caller {
            actor_id,
            call_fn,
            downgrade_fn,
        }
    }
}

impl<M: Message> Clone for Caller<M> {
    fn clone(&self) -> Self {
        Caller {
            actor_id: self.actor_id,
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
