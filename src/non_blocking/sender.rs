use futures::{channel::mpsc, SinkExt};
use std::{marker::PhantomData, sync::Arc};

use super::{ActorOuter, AsyncActor, AsyncAddr, AsyncHandler, Payload};
use crate::ActorResult;

#[derive(Clone)]
pub struct AsyncSender<M> {
    tx: Arc<mpsc::Sender<Payload>>,
    actor: ActorOuter<M>,
    marker: PhantomData<M>,
}

impl<M, A> From<AsyncAddr<A>> for AsyncSender<M>
where
    A: AsyncHandler<M> + 'static,
    A: AsyncActor + 'static,
{
    fn from(AsyncAddr { ctx, actor }: AsyncAddr<A>) -> Self {
        AsyncSender {
            tx: ctx.tx.clone(),
            actor: actor.clone(),
            marker: PhantomData,
        }
    }
}

impl<M> AsyncSender<M> {
    pub async fn send(&self, msg: M) -> ActorResult<()>
    where
        M: Send + 'static,
    {
        let actor = self.actor.clone();
        let mut tx = Arc::unwrap_or_clone(self.tx.clone());
        tx.send(Payload::Exec(Box::new(move || {
            Box::pin(async move {
                let writable_actor: Option<&mut AsyncHandler<M>> =
                    actor.write().await.downcast_mut();

                actor.write().await.handle(msg).await?;
                Ok(())
            })
        })))
        .await?;
        Ok(())
    }
}
