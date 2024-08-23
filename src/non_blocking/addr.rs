use std::sync::Arc;

use super::{ActorOuter, AsyncActor, AsyncContext, AsyncHandler, AsyncSender};
use crate::ActorResult;

pub struct AsyncAddr<A: AsyncActor> {
    pub(crate) ctx: Arc<AsyncContext>,
    pub(crate) actor: ActorOuter<A>,
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

    pub fn sender<M>(&self) -> AsyncSender<M>
    where
        A: AsyncHandler<M> + 'static,
        M: Send + 'static,
    {
        (*self).clone().into()
    }

    // TODO: write send method at a time when you still know A but then hand out a

    pub fn sender_fn<M>(&self, f: impl Fn(M) -> ActorResult<()> + Send + Sync + 'static) -> AsyncSender<M>
    where
        M: Send + 'static,
    {
        let actor = self.actor.clone();
        let tx = self.ctx.tx.clone();
    }
}
