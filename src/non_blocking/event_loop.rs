use super::{AsyncActor, AsyncAddr, AsyncContext};
use crate::{error::ActorError::SpawnError, ActorResult};

use std::{future::Future, pin::Pin, sync::Arc};

use async_lock::RwLock;
use futures::{
    channel::mpsc::{self},
    StreamExt,
};

type Exec = dyn FnOnce() -> Pin<Box<dyn Future<Output = ActorResult<()>> + Send>> + Send + 'static;

pub enum Payload {
    Exec(Box<Exec>),
    Stop,
}

impl<F> From<F> for Payload
where
    F: FnOnce() -> Pin<Box<dyn Future<Output = ActorResult<()>> + Send>> + Send + 'static,
{
    fn from(f: F) -> Self {
        Payload::Exec(Box::new(f))
    }
}

type BoxedAsyncActor = Arc<RwLock<dyn AsyncActor>>;

impl AsyncActor for BoxedAsyncActor {
    fn started(&mut self) -> Pin<Box<dyn std::future::Future<Output = ActorResult<()>> + Send>> {
        let actor = self.clone();
        Box::pin(async move { actor.as_ref().write().await.started().await })
    }

    fn stopped(&mut self) -> Pin<Box<dyn std::future::Future<Output = ActorResult<()>> + Send>> {
        let actor = self.clone();
        Box::pin(async move { actor.as_ref().write().await.stopped().await })
    }
}

#[derive(Default)]
pub struct AsyncEventLoop {
    pub ctx: AsyncContext,
}

impl AsyncEventLoop {
    pub async fn start<A>(mut self, actor: A) -> ActorResult<AsyncAddr<A>>
    where
        A: AsyncActor + Send + Sync + 'static,
    {
        let actor = Arc::new(RwLock::new(actor));
        self.spawn(actor.clone()).await.unwrap();
        Ok(AsyncAddr {
            ctx: Arc::new(self.ctx),
            actor,
        })
    }

    pub(crate) fn async_loop(
        mut actor: BoxedAsyncActor,
        mut rx: mpsc::Receiver<Payload>,
    ) -> impl Future<Output = ActorResult<()>> {
        async move {
            actor.started().await?;
            eprintln!("waiting for events");
            while let Some(payload) = rx.next().await {
                match payload {
                    Payload::Exec(exec) => {
                        eprintln!("received Exec");
                        exec().await?;
                    }
                    Payload::Stop => break,
                }
            }
            actor.stopped().await?;
            Ok(())
        }
    }

    pub async fn spawn(&mut self, actor: BoxedAsyncActor) -> ActorResult<()> {
        let Some(rx) = self.ctx.take_rx() else {
            eprintln!("Cannot spawn context");
            return Err(SpawnError);
        };
        Self::async_loop(actor, rx).await?;
        todo!();
    }
}
