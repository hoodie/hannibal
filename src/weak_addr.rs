use crate::{
    channel_wrapper::SendFn, context::RunningFuture, error::bail, Actor, ActorId, Addr, Handler,
    Message, Result,
};
use std::{
    hash::{Hash, Hasher},
    sync::Weak,
};

/// Weak version of [`Addr<A>`].
///
/// This address will not prolong the lifetime of the actor.
/// In order to use a [`WeakAddr<A>`] you need to "upgrade" it to a proper [`Addr<A>`].
pub struct WeakAddr<A> {
    pub(crate) actor_id: ActorId,
    pub(crate) tx: Weak<dyn SendFn<A>>,
    pub(crate) rx_exit: Option<RunningFuture>,
}

impl<A> std::fmt::Debug for WeakAddr<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeakAddr")
            .field("actor_id", &self.actor_id)
            .field("rx_exit", &self.rx_exit)
            .finish_non_exhaustive()
    }
}

impl<A> PartialEq for WeakAddr<A> {
    fn eq(&self, other: &Self) -> bool {
        self.actor_id == other.actor_id
    }
}

impl<A> Hash for WeakAddr<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.actor_id.hash(state);
    }
}

impl<A> WeakAddr<A> {
    /// Attempts to turn a [`WeakAddr<A>`] into an [`Addr<A>`].
    ///
    /// If the original [`Addr<A>`] has already been dropped this method will return [`None`]
    pub fn upgrade(&self) -> Option<Addr<A>> {
        self.tx.upgrade().map(|tx| Addr {
            actor_id: self.actor_id,
            tx,
            rx_exit: self.rx_exit.clone(),
        })
    }

    /// Try to upgrade to [`Addr`] and call [`Addr::send`]
    pub fn upgrade_send<T: Message<Result = ()>>(&self, msg: T) -> Result<()>
    where
        A: Handler<T>,
    {
        if let Some(addr) = self.upgrade() {
            addr.send(msg)
        } else {
            bail!("cannot upgrade");
        }
    }
}

impl<A> Clone for WeakAddr<A> {
    fn clone(&self) -> Self {
        Self {
            actor_id: self.actor_id,
            tx: self.tx.clone(),
            rx_exit: self.rx_exit.clone(),
        }
    }
}

impl<A: Actor> WeakAddr<A> {
    /// Returns the id of the actor.
    pub fn actor_id(&self) -> ActorId {
        self.actor_id
    }
}
