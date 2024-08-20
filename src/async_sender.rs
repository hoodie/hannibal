use async_lock::RwLock;
use futures::{channel::mpsc, SinkExt};
use std::{marker::PhantomData, sync::Arc};

use crate::{
    async_addr::AsyncAddr, async_event_loop::Payload, error::ActorError, ActorResult, AsyncHandler,
};

#[derive(Clone)]
pub struct AsyncSender<M> {
    tx: Arc<mpsc::Sender<Payload>>,
    actor: Arc<RwLock<dyn AsyncHandler<M>>>,
    marker: PhantomData<M>,
}

impl<M, A> From<AsyncAddr<A>> for AsyncSender<M>
where
    A: AsyncHandler<M> + 'static,
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
                actor.write().await.handle(msg).await?;
                Ok(())
            })
        })))
        .await?;
        Ok(())
    }

    // pub fn downgrade(&self) -> WeakSender<M> {
    //     WeakSender {
    //         tx: Arc::downgrade(&self.tx),
    //         actor: Arc::downgrade(&self.actor),
    //         marker: PhantomData,
    //     }
    // }
}

// #[derive(Clone)]
// pub struct WeakSender<M> {
//     tx: Weak<mpsc::Sender<Payload>>,
//     actor: Weak<dyn Handler<M>>,
//     marker: PhantomData<M>,
// }

// impl<M> WeakSender<M> {
//     pub async fn try_send(&self, msg: M) -> bool
//     where
//         M: Send + 'static,
//     {
//         if let Some((tx, actor)) = self.tx.upgrade().zip(self.actor.upgrade()) {
//             let mut tx = Arc::unwrap_or_clone(tx.clone());
//             tx.send(Payload::Exec(Box::new(move || {
//                 Box::pin(async move { actor.handle(msg) })
//             })))
//             .await;
//             true
//         } else {
//             eprintln!("Actor is dead");
//             false
//         }
//     }
// }
