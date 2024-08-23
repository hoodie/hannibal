use std::sync::{Arc, Weak};

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
    send_fn: Box<dyn SenderFn<M>>,
}

impl<M: Message> WeakSender<M> {
    pub fn try_send(&self, msg: M) -> Result<()> {
        self.send_fn.send(msg)
    }
}

impl<M, A> From<ChanTx<A>> for WeakSender<M>
where
    A: Actor + Handler<M>,
    M: Message<Result = ()>,
{
    fn from(tx: ChanTx<A>) -> Self {
        let tx: Weak<_> = Arc::downgrade(&tx);
        let send_fn = Box::new(move |msg| {
            let Some(tx) = tx.upgrade() else {
                return Err(anyhow::anyhow!("failed to upgrade"));
            };

            tx.send(ActorEvent::exec(move |actor, ctx| {
                Box::pin(Handler::handle(&mut *actor, ctx, msg))
            }))?;

            Ok(())
        });

        WeakSender { send_fn }
    }
}
