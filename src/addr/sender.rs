use std::sync::Arc;

use dyn_clone::DynClone;

use crate::{channel::ChanTx, Actor, Handler};

use super::{ActorEvent, Message, Result};

trait SenderFn<T: Message>: 'static + Send + Sync + DynClone {
    fn send(&self, msg: T) -> Result<()>;
}

impl<F, M: Message> SenderFn<M> for F
where
    F: Fn(M) -> Result<()>,
    F: 'static + Send + Sync + DynClone,
{
    fn send(&self, msg: M) -> Result<()> {
        self(msg)
    }
}

/// Sender of a specific message type.
pub struct Sender<M: Message> {
    send_fn: Box<dyn SenderFn<M>>,
}

impl<M: Message> Sender<M> {
    pub fn send(&self, msg: M) -> Result<()> {
        self.send_fn.send(msg)
    }
}

impl<M: Message<Result = ()>, A> From<ChanTx<A>> for Sender<M>
where
    A: Actor + Handler<M>,
{
    fn from(tx: ChanTx<A>) -> Self {
        let tx: Arc<_> = tx.clone();
        let send_fn = Box::new(move |msg| {
            tx.send(ActorEvent::exec(move |actor, ctx| {
                Box::pin(Handler::handle(&mut *actor, ctx, msg))
            }))?;

            Ok(())
        });

        Sender { send_fn }
    }
}

/// WeakSender of a specific message type.
pub struct WeakSender<M: Message> {
    upgrade: Box<dyn UpgradeFn<M>>,
}

impl<M: Message> WeakSender<M> {
    pub fn upgrade(&self) -> Option<Sender<M>> {
        self.upgrade.upgrade()
    }

    pub fn try_send(&self, msg: M) -> Option<Result<()>> {
        if let Some(sender) = self.upgrade.upgrade() {
            Some(sender.send(msg))
        } else {
            None
        }
    }
}

impl<M, A> From<ChanTx<A>> for WeakSender<M>
where
    A: Actor + Handler<M>,
    M: Message<Result = ()>,
{
    fn from(tx: ChanTx<A>) -> Self {
        let weak_tx = Arc::downgrade(&tx);

        let upgrade = Box::new(move || weak_tx.upgrade().map(|tx| Sender::from(tx)));

        WeakSender { upgrade }
    }
}

impl<M: Message> Clone for Sender<M> {
    fn clone(&self) -> Self {
        Sender {
            send_fn: dyn_clone::clone_box(&*self.send_fn),
        }
    }
}

impl<M: Message> Clone for WeakSender<M> {
    fn clone(&self) -> Self {
        WeakSender {
            upgrade: dyn_clone::clone_box(&*self.upgrade),
        }
    }
}

trait UpgradeFn<M: Message>: Send + Sync + 'static + DynClone {
    fn upgrade(&self) -> Option<Sender<M>>;
}

impl<F, M> UpgradeFn<M> for F
where
    F: Fn() -> Option<Sender<M>>,
    F: 'static + Send + Sync + DynClone,
    M: Message,
{
    fn upgrade(&self) -> Option<Sender<M>> {
        self()
    }
}
