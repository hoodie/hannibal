use dyn_clone::DynClone;

use std::sync::Arc;

use crate::{channel::ChanTx, Actor, ActorId, Error, Handler};

use super::{caller::Caller, Addr, Message, Result};

/// Weak version of [`WeakCaller<M>`].
///
/// Unlike [`Caller<A>`], `WeakCaller` has a *weak* reference to the recipient of the message type,
/// and so will *not* prevent an actor from stopping if all [`crate::Addr`]'s have been dropped elsewhere.
///
/// This struct is created via [`Addr::weak_caller()`](`super::Addr::weak_caller`).
/// You can also create a `WeakCaller` by downgrading a `Caller` using [`Caller::downgrade()`].
pub struct WeakCaller<M: Message> {
    pub(super) actor_id: ActorId,
    pub(super) upgrade: Box<dyn UpgradeFn<M>>,
}

impl<M: Message> WeakCaller<M> {
    /// Upgrade this `WeakCaller` to a `Caller` if the recipient actor is still alive.
    pub fn upgrade(&self) -> Option<Caller<M>> {
        self.upgrade.upgrade()
    }

    /// Try to call the actor with the given message and wait for the result.
    pub async fn try_call(&self, msg: M) -> Result<M::Result> {
        if let Some(caller) = self.upgrade.upgrade() {
            caller.call(msg).await
        } else {
            Err(Error::AlreadyStopped)
        }
    }
}

impl<M: Message> PartialEq for WeakCaller<M> {
    fn eq(&self, other: &Self) -> bool {
        self.actor_id.eq(&other.actor_id)
    }
}

impl<M: Message<Result = ()>, A> From<Addr<A>> for WeakCaller<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        addr.pieces().into()
    }
}

impl<M: Message, A> From<(ActorId, ChanTx<A>)> for WeakCaller<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn from((actor_id, tx): (ActorId, ChanTx<A>)) -> Self {
        let weak_tx = Arc::downgrade(&tx);

        let upgrade = Box::new(move || weak_tx.upgrade().map(|tx| Caller::from((actor_id, tx))));

        WeakCaller { actor_id, upgrade }
    }
}

impl<M: Message> Clone for WeakCaller<M> {
    fn clone(&self) -> Self {
        WeakCaller {
            actor_id: self.actor_id,
            upgrade: dyn_clone::clone_box(&*self.upgrade),
        }
    }
}

pub(super) trait UpgradeFn<M: Message>: Send + Sync + 'static + DynClone {
    fn upgrade(&self) -> Option<Caller<M>>;
}

impl<F, M> UpgradeFn<M> for F
where
    F: Fn() -> Option<Caller<M>>,
    F: 'static + Send + Sync + Clone,
    M: Message,
{
    fn upgrade(&self) -> Option<Caller<M>> {
        self()
    }
}
