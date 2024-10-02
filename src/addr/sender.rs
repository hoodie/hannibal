use dyn_clone::DynClone;

use std::sync::{Arc, Weak};

use crate::{channel::ChanTx, Actor, Handler};

use super::{weak_sender::WeakSender, Addr, Message, Payload, Result};

pub struct Sender<M: Message<Result = ()>> {
    send_fn: Box<dyn SenderFn<M>>,
    downgrade_fn: Box<dyn DowngradeFn<M>>,
}

impl<M: Message<Result = ()>> Sender<M> {
    pub fn send(&self, msg: M) -> Result<()> {
        self.send_fn.send(msg)
    }

    pub fn downgrade(&self) -> WeakSender<M> {
        self.downgrade_fn.downgrade()
    }

    pub(crate) fn from_tx<A>(tx: ChanTx<A>) -> Self
    where
        A: Actor + Handler<M>,
    {
        let weak_tx: Weak<_> = Arc::downgrade(&tx);

        let send_fn = Box::new(move |msg| {
            tx.send(Payload::task(move |actor, ctx| {
                Box::pin(Handler::handle(&mut *actor, ctx, msg))
            }))
        });

        let upgrade = Box::new(move || weak_tx.upgrade().map(|tx| Sender::from_tx(tx)));

        let downgrade_fn = Box::new(move || WeakSender {
            upgrade: upgrade.clone(),
        });

        Sender {
            send_fn,
            downgrade_fn,
        }
    }
}

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

impl<M: Message<Result = ()>, A> From<&Addr<A>> for Sender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: &Addr<A>) -> Self {
        Sender::from_tx(addr.payload_tx.to_owned())
    }
}

impl<M: Message<Result = ()>> Clone for Sender<M> {
    fn clone(&self) -> Self {
        Sender {
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
