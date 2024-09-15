use dyn_clone::DynClone;

use std::sync::Arc;

use crate::{
    channel::{ChanTx, WeakChanTx},
    Actor, ActorId, Error, Handler,
};

use super::{sender::Sender, Addr, Message, Result};

/// Weak version of [`Sender<M>`].
///
/// Unlike [`Sender<A>`], `WeakSender` has a *weak* reference to the recipient of the message type,
/// and so will *not* prevent an actor from stopping if all [`crate::Addr`]'s have been dropped elsewhere.
///
/// This struct is created via [`Addr::weak_sender()`](`super::Addr::weak_sender`).
/// You can also create a `WeakSender` from a [`Sender`] using [`Sender::downgrade()`].
pub struct WeakSender<M> {
    pub(super) actor_id: ActorId,
    pub(super) upgrade: Box<dyn UpgradeFn<M>>,
}

impl<M: Message<Result = ()>> WeakSender<M> {
    /// Upgrade this `WeakSender` to a `Sender` if the recipient actor is still alive.
    pub fn upgrade(&self) -> Option<Sender<M>> {
        self.upgrade.upgrade()
    }

    /// Try to send a message to the recipient actor.
    pub fn try_send(&self, msg: M) -> Result<()> {
        if let Some(sender) = self.upgrade.upgrade() {
            sender.send(msg)
        } else {
            Err(Error::AlreadyStopped)
        }
    }
}

impl<M: Message<Result = ()>> std::fmt::Debug for WeakSender<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("WeakSender<{}>", M::TYPE_NAME))
            .field("actor_id", &self.actor_id)
            .finish()
    }
}

impl<M: Message<Result = ()>> PartialEq for WeakSender<M> {
    fn eq(&self, other: &Self) -> bool {
        self.actor_id.eq(&other.actor_id)
    }
}

impl<M: Message<Result = ()>, A> From<Addr<A>> for WeakSender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        addr.pieces().into()
    }
}

impl<M, A> From<(ActorId, ChanTx<A>)> for WeakSender<M>
where
    A: Actor + Handler<M>,
    M: Message<Result = ()>,
{
    fn from((actor_id, tx): (ActorId, ChanTx<A>)) -> Self {
        (actor_id, Arc::downgrade(&tx)).into()
    }
}

impl<M, A> From<(ActorId, WeakChanTx<A>)> for WeakSender<M>
where
    A: Actor + Handler<M>,
    M: Message<Result = ()>,
{
    fn from((actor_id, weak_tx): (ActorId, WeakChanTx<A>)) -> Self {
        let upgrade = Box::new(move || weak_tx.upgrade().map(|tx| Sender::from((actor_id, tx))));

        WeakSender { actor_id, upgrade }
    }
}

impl<M: Message<Result = ()>> Clone for WeakSender<M> {
    fn clone(&self) -> Self {
        WeakSender {
            actor_id: self.actor_id,
            upgrade: dyn_clone::clone_box(&*self.upgrade),
        }
    }
}

pub(super) trait UpgradeFn<M: Message<Result = ()>>:
    Send + Sync + 'static + DynClone
{
    fn upgrade(&self) -> Option<Sender<M>>;
}

impl<F, M> UpgradeFn<M> for F
where
    F: Fn() -> Option<Sender<M>>,
    F: 'static + Send + Sync + Clone,
    M: Message<Result = ()>,
{
    fn upgrade(&self) -> Option<Sender<M>> {
        self()
    }
}
