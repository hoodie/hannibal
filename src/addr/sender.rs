use anyhow::anyhow;

use crate::{channel::ChanTx, Actor, Handler};

use super::{ActorEvent, Message, Result};
use std::sync::{Arc, Weak};

pub(crate) trait SenderFn<T: Message>: 'static + Send + Sync {
    fn send(&self, msg: T) -> Result<()>;
}

impl<F, T> SenderFn<T> for F
where
    F: Fn(T) -> Result<()>,
    F: 'static + Send + Sync + Clone,
    T: Message,
{
    fn send(&self, msg: T) -> Result<()> {
        self(msg)
    }
}

/// Sender of a specific message type.
#[derive(Clone)]
pub struct Sender<T>
where
    T: Message,
{
    pub(crate) send_fn: Arc<dyn SenderFn<T>>,
}

impl<T, A> From<ChanTx<A>> for Sender<T>
where
    A: Actor,
    A: Handler<T>,
    T: Message<Result = ()>,
{
    fn from(tx: ChanTx<A>) -> Self {
        let send_fn = Arc::new(move |msg| {
            tx.send(ActorEvent::exec(move |actor, ctx| {
                Box::pin(Handler::handle(&mut *actor, ctx, msg))
            }))?;
            Ok(())
        });

        Sender { send_fn }
    }
}

/// `WeakSender` of a specific message type. You need to upgrade it to a `Sender` before you can use it.
///
/// Like [`WeakCaller<T>`](`super::WeakCaller<T>`), Sender has a weak reference to the recipient of the message type,
/// and so will not prevent an actor from stopping if all [`Addr`](`crate::Addr`)'s have been dropped elsewhere.
/// This allows it to be used in the `send_later` and `send_interval` actor functions,
/// and not keep the actor alive indefinitely even after all references to it have been dropped (unless `ctx.stop()` is called from within)

#[derive(Clone)]
pub struct WeakSender<T: Message> {
    pub(crate) send_fn: Weak<dyn SenderFn<T>>,
}

impl<T: Message<Result = ()>> Sender<T> {
    pub fn send(&self, msg: T) -> Result<()> {
        self.send_fn.send(msg)
    }
    pub fn downgrade(&self) -> WeakSender<T> {
        WeakSender {
            send_fn: Arc::downgrade(&self.send_fn),
        }
    }
}

impl<T: Message> WeakSender<T> {
    pub fn upgrade(&self) -> Option<Sender<T>> {
        self.send_fn.upgrade().map(|send_fn| Sender { send_fn })
    }

    pub fn can_upgrade(&self) -> bool {
        Weak::strong_count(&self.send_fn) > 1
    }

    pub fn try_send(&self, msg: T) -> Result<()> {
        let send_fn = self
            .send_fn
            .upgrade()
            .ok_or_else(|| anyhow!("Actor dropped"))?;
        send_fn.send(msg)
    }
}
