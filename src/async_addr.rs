use std::sync::Arc;

use async_lock::RwLock;

use crate::{
    AsyncHandler, non_blocking::{AsyncContext, AsyncSender},
    ActorResult, AsyncActor,
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
    pub async fn send<M>(&self, msg: M) -> ActorResult<()>
    where
        A: AsyncHandler<M> + 'static,
        M: Send + Sync + 'static,
    {
        self.ctx.send(msg, self.actor.clone()).await?;
        Ok(())
    }

    pub async fn stop(&self) -> ActorResult<()> {
        self.ctx.stop().await
    }

    // pub fn downgrade(&self) -> WeakAddr<A> {
    //     WeakAddr {
    //         ctx: Arc::downgrade(&self.ctx),
    //         actor: Arc::downgrade(&self.actor),
    //     }
    // }

    pub fn sender<M>(&self) -> AsyncSender<M>
    where
        A: AsyncHandler<M> + 'static,
        M: Send + 'static,
    {
        (*self).clone().into()
    }
}
