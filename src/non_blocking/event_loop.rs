#![allow(unused_imports)]
use super::{AsyncActor, AsyncAddr, AsyncContext};
use crate::ActorResult;

use std::{future::Future, pin::Pin, sync::Arc};

use async_lock::RwLock;
use futures::{
    channel::mpsc::{self, Receiver, Sender, UnboundedReceiver, UnboundedSender},
    FutureExt, StreamExt,
};

use futures::task::LocalSpawnExt;

type Exec = dyn FnOnce() -> Pin<Box<dyn Future<Output = ActorResult<()>> + Send>> + Send + 'static;

pub enum Payload {
    Exec(Box<Exec>),
    Stop,
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
    pub fn start<A>(mut self, actor: A) -> AsyncAddr<A>
    where
        A: AsyncActor + Send + Sync + 'static,
    {
        let actor = Arc::new(RwLock::new(actor));
        self.spawn(actor.clone()).unwrap();
        AsyncAddr {
            ctx: Arc::new(self.ctx),
            actor,
        }
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

    #[allow(unused)]
    pub fn spawn(&mut self, actor: BoxedAsyncActor) -> Result<(), String> {
        let Some(rx) = self.ctx.take_rx() else {
            eprintln!("Cannot spawn context");
            return Err("Cannot spawn context".to_string());
        };
        Self::async_loop(actor, rx);
        todo!();
        Ok(())
    }
}
