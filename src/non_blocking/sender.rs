use futures::{channel::mpsc, SinkExt};
use std::{marker::PhantomData, sync::Arc};

use super::{ActorOuter, Actor, Addr, Handler, Payload};
use crate::ActorResult;

#[derive(Clone)]
pub struct Sender<M> {
    tx: Arc<mpsc::Sender<Payload>>,
    actor: ActorOuter<M>,
    marker: PhantomData<M>,
}

impl<M, A> From<Addr<A>> for Sender<M>
where
    A: Handler<M> + 'static,
    A: Actor + 'static,
{
    fn from(Addr { ctx, actor }: Addr<A>) -> Self {
        Sender {
            tx: ctx.tx.clone(),
            actor: actor.clone(),
            marker: PhantomData,
        }
    }
}

impl<M> Sender<M> {
    pub async fn send(&self, msg: M) -> ActorResult<()>
    where
        M: Send + 'static,
    {
        let actor = self.actor.clone();
        let mut tx = Arc::unwrap_or_clone(self.tx.clone());
        tx.send(Payload::Exec(Box::new(move || {
            Box::pin(async move {
                let writable_actor: Option<&mut Handler<M>> =
                    actor.write().await.downcast_mut();

                actor.write().await.handle(msg).await?;
                Ok(())
            })
        })))
        .await?;
        Ok(())
    }
}
