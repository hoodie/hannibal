use dyn_clone::DynClone;

use std::sync::{Arc, Weak};

use crate::{channel::ChanTx, Actor, ActorId, Handler};

use super::{weak_sender::WeakSender, ActorEvent, Addr, Message, Result};

trait SenderFn<M: Message<Result = ()>>: 'static + Send + Sync + DynClone {
    fn send(&self, msg: M) -> Result<()>;
}

impl<F, M: Message<Result = ()>> SenderFn<M> for F
where
    F: Fn(M) -> Result<()>,
    F: 'static + Send + Sync + Clone,
{
    fn send(&self, msg: M) -> Result<()> {
        self(msg)
    }
}

/// Lets you send messages to an actor.
///
/// A `Sender` is for messages that don't require a response.
/// This struct is created via [`Addr::sender()`](`super::Addr::sender`).
///
/// Like [`Addr<A>`](`super::Addr<A>`), `Sender` has a *strong* reference to the recipient of the message type,
/// and so will prevent an actor from stopping if all [`crate::Addr`]'s have been dropped elsewhere.
///
/// You can downgrade a `Sender` to a [`WeakSender`] to avoid creating a reference cycle.
pub struct Sender<M: Message<Result = ()>> {
    actor_id: ActorId,
    send_fn: Box<dyn SenderFn<M>>,
    downgrade_fn: Box<dyn DowngradeFn<M>>,
}

impl<M: Message<Result = ()>> Sender<M> {
    /// Send the actor the given message.
    pub fn send(&self, msg: M) -> Result<()> {
        self.send_fn.send(msg)
    }

    /// Downgrade to a [`WeakSender`] to avoid creating a reference cycle.
    pub fn downgrade(&self) -> WeakSender<M> {
        self.downgrade_fn.downgrade()
    }
}

impl<M: Message<Result = ()>> PartialEq for Sender<M> {
    fn eq(&self, other: &Self) -> bool {
        self.actor_id.eq(&other.actor_id)
    }
}

impl<M: Message<Result = ()>, A> From<Addr<A>> for Sender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        addr.pieces().into()
    }
}

impl<M: Message<Result = ()>, A> From<(ActorId, ChanTx<A>)> for Sender<M>
where
    A: Actor + Handler<M>,
{
    fn from((actor_id, tx): (ActorId, ChanTx<A>)) -> Self {
        let weak_tx: Weak<_> = Arc::downgrade(&tx);

        let send_fn = Box::new(move |msg| {
            tx.send(ActorEvent::exec(move |actor, ctx| {
                Box::pin(Handler::handle(&mut *actor, ctx, msg))
            }))
        });

        let upgrade = Box::new(move || weak_tx.upgrade().map(|tx| Sender::from((actor_id, tx))));
        let downgrade_fn = Box::new(move || WeakSender {
            actor_id,
            upgrade: upgrade.clone(),
        });

        Sender {
            actor_id,
            send_fn,
            downgrade_fn,
        }
    }
}

impl<M: Message<Result = ()>> Clone for Sender<M> {
    fn clone(&self) -> Self {
        Sender {
            actor_id: self.actor_id,
            send_fn: dyn_clone::clone_box(&*self.send_fn),
            downgrade_fn: dyn_clone::clone_box(&*self.downgrade_fn),
        }
    }
}

trait DowngradeFn<M: Message<Result = ()>>: Send + Sync + 'static + DynClone {
    fn downgrade(&self) -> WeakSender<M>;
}

impl<F, M> DowngradeFn<M> for F
where
    F: Fn() -> WeakSender<M>,
    F: 'static + Send + Sync + Clone,
    M: Message<Result = ()>,
{
    fn downgrade(&self) -> WeakSender<M> {
        self()
    }
}
