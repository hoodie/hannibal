use dyn_clone::DynClone;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
};

use crate::{
    channel::{ChanTx, ForceChanTx},
    context::ContextID,
    Actor, Handler,
};

use super::{weak_sender::WeakSender, Addr, Message, Payload, Result};

pub struct Sender<M: Message<Result = ()>> {
    send_fn: Box<dyn SenderFn<M>>,
    force_send_fn: Box<dyn ForceSenderFn<M>>,
    downgrade_fn: Box<dyn DowngradeFn<M>>,
    id: ContextID,
}

impl<M: Message<Result = ()>> Sender<M> {
    pub fn send(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        self.send_fn.send(msg)
    }

    #[deprecated(note = "use send_safe")]
    pub fn force_send(&self, msg: M) -> Result<()> {
        self.force_send_fn.send(msg)
    }

    pub fn downgrade(&self) -> WeakSender<M> {
        self.downgrade_fn.downgrade()
    }

    pub(crate) fn new<A>(tx: ChanTx<A>, force_tx: ForceChanTx<A>, id: ContextID) -> Self
    where
        A: Actor + Handler<M>,
    {
        let weak_tx: Weak<_> = Arc::downgrade(&tx);
        let weak_force_tx: Weak<_> = Arc::downgrade(&force_tx);

        let send_fn = Box::new(move |msg| {
            tx.send(Payload::task(move |actor, ctx| {
                Box::pin(Handler::handle(&mut *actor, ctx, msg))
            }))
        });

        let force_send_fn = Box::new(move |msg| {
            force_tx.send(Payload::task(move |actor, ctx| {
                Box::pin(Handler::handle(&mut *actor, ctx, msg))
            }))
        });

        let upgrade = Box::new(move || {
            weak_tx
                .upgrade()
                .zip(weak_force_tx.upgrade())
                .map(|(tx, force_tx)| Sender::new(tx, force_tx, id))
        });

        let downgrade_fn = Box::new(move || WeakSender {
            upgrade: upgrade.clone(),
            id,
        });

        Sender {
            id,
            send_fn,
            force_send_fn,
            downgrade_fn,
        }
    }
}

trait ForceSenderFn<M: Message<Result = ()>>: 'static + Send + Sync + DynClone {
    fn send(&self, msg: M) -> Result<()>;
}

impl<F, M: Message<Result = ()>> ForceSenderFn<M> for F
where
    F: Fn(M) -> Result<()>,
    F: 'static + Send + Sync + Clone,
{
    fn send(&self, msg: M) -> Result<()> {
        self(msg)
    }
}

trait SenderFn<M: Message<Result = ()>>: 'static + Send + Sync + DynClone {
    fn send(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;
}

impl<F, M: Message<Result = ()>> SenderFn<M> for F
where
    F: Fn(M) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>,
    F: 'static + Send + Sync + Clone,
{
    fn send(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        self(msg)
    }
}

impl<M: Message<Result = ()>, A> From<Addr<A>> for Sender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        Sender::new(
            addr.payload_tx.to_owned(),
            addr.payload_force_tx.to_owned(),
            addr.context_id,
        )
    }
}

impl<M: Message<Result = ()>, A> From<&Addr<A>> for Sender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: &Addr<A>) -> Self {
        Sender::new(
            addr.payload_tx.to_owned(),
            addr.payload_force_tx.to_owned(),
            addr.context_id,
        )
    }
}

impl<M: Message<Result = ()>> Clone for Sender<M> {
    fn clone(&self) -> Self {
        Sender {
            id: self.id,
            send_fn: dyn_clone::clone_box(&*self.send_fn),
            force_send_fn: dyn_clone::clone_box(&*self.force_send_fn),
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
