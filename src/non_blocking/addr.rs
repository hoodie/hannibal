use std::sync::Arc;

use super::{Actor, Context, Handler, Sender};
use crate::ActorResult;

pub struct Addr<A: Actor> {
    pub(crate) ctx: Arc<Context<A>>,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Addr {
            ctx: self.ctx.clone(),
        }
    }
}

impl<A: Actor> Addr<A> {
    pub async fn send<M>(&self, msg: M) -> ActorResult<()>
    where
        A: Handler<M> + 'static,
        M: Send + Sync + 'static,
    {
        self.ctx.send(msg).await?;
        Ok(())
    }

    pub async fn stop(&self) -> ActorResult<()> {
        self.ctx.stop().await
    }

    pub fn sender<M>(&self) -> Sender<M>
    where
        A: Handler<M> + 'static,
        M: Send + 'static,
    {
        (*self).clone().into()
    }
}
