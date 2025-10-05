#![allow(unused_imports)]
use dyn_clone::DynClone;
use std::sync::{Arc, RwLock, Weak};

use crate::{
    ActorResult, Context, Sender, actor::Actor, channel::ChanTx, error::ActorError,
    handler::Handler, payload::Payload,
};

pub struct Addr<A> {
    pub(crate) payload_tx: ChanTx<A>,
}

pub struct WeakAddr<A: Actor> {
    pub(super) upgrade: Box<dyn UpgradeFn<A>>,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Addr {
            payload_tx: Arc::clone(&self.payload_tx),
        }
    }
}

impl<A: Actor> Addr<A> {
    pub fn send<M>(&self, msg: M) -> ActorResult<()>
    where
        A: Handler<M> + 'static,
        M: Send + Sync + 'static,
    {
        self.payload_tx.send(Payload::task(move |actor, ctx| {
            Handler::handle(
                actor, // ctx,
                msg,
            )
        }));
        Ok(())
    }

    // pub fn stop(&self) -> ActorResult<()> {
    //     self.ctx.stop()
    // }

    pub fn sender<M>(&self) -> Sender<M>
    where
        A: Handler<M> + 'static,
        M: Send + 'static,
    {
        (*self).clone().into()
    }
}

impl<A: Actor> From<&Addr<A>> for WeakAddr<A> {
    fn from(addr: &Addr<A>) -> Self {
        let weak_tx = Arc::downgrade(&addr.payload_tx);
        // let weak_force_tx = Arc::downgrade(&addr.payload_force_tx);
        // let context_id = addr.context_id;
        // let running = addr.running.clone();
        // let running_inner = addr.running.clone();
        let upgrade = Box::new(move || {
            // let running = running_inner.clone();
            weak_tx.upgrade().map(|(payload_tx)| Addr { payload_tx })
        });

        WeakAddr {
            context_id,
            upgrade,
            running,
        }
    }
}
pub(super) trait UpgradeFn<A: Actor>: Send + Sync + 'static + DynClone {
    fn upgrade(&self) -> Option<Addr<A>>;
}

impl<A: Actor> WeakAddr<A> {
    // pub fn try_send<M>(&self, msg: M) -> ActorResult<()>
    // where
    //     A: Handler<M> + 'static,
    //     M: Send + Sync + 'static,
    // {
    //     if let Some(addr) = self.upgrade() {
    //         addr.send(msg)?;
    //         Ok(())
    //     } else {
    //         Err(ActorError::AlreadyStopped)
    //     }
    // }
}
