#![allow(unused_imports)]
use std::sync::Arc;

use async_lock::RwLock;

use crate::{
    error::ActorError, non_blocking::AsyncContext, ActorResult, AsyncActor, Context, Handler,
    Sender,
};

pub struct AsyncAddr<A: AsyncActor> {
    pub(crate) ctx: Arc<AsyncContext>,
    pub(crate) actor: Arc<RwLock<A>>,
}

impl<A: AsyncActor> Clone for AsyncAddr<A> {
    fn clone(&self) -> Self {
        AsyncAddr {
            ctx: self.ctx.clone(),
            actor: self.actor.clone(),
        }
    }
}

impl<A: AsyncActor> AsyncAddr<A> {
    pub fn send<M>(&self, msg: M) -> ActorResult<()>
    where
        A: Handler<M> + 'static,
        M: Send + Sync + 'static,
    {
        self.ctx.send(msg, self.actor.clone())?;
        Ok(())
    }

    pub fn stop(&self) -> ActorResult<()> {
        self.ctx.stop()
    }

    // pub fn downgrade(&self) -> WeakAddr<A> {
    //     WeakAddr {
    //         ctx: Arc::downgrade(&self.ctx),
    //         actor: Arc::downgrade(&self.actor),
    //     }
    // }

    pub fn sender<M>(&self) -> Sender<M>
    where
        A: Handler<M> + 'static,
        M: Send + 'static,
    {
        (*self).clone().into()
    }
}
