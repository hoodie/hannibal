use super::{addr, Actor, Addr, Context};
use crate::{error::ActorError::SpawnError, ActorResult};

use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc};

use futures::{
    channel::mpsc::{self},
    StreamExt,
};

type Execution<A: Actor> = dyn for<'a> FnOnce(&'a mut A) -> Pin<Box<dyn Future<Output = ActorResult<()>> + Send>>
    + Send
    + 'static;

pub enum Payload<A> {
    Exec(Box<Execution<A>>),
    Stop,
}

impl<F> From<F> for Payload<_>
where
    F: FnOnce() -> Pin<Box<dyn Future<Output = ActorResult<()>> + Send>> + Send + 'static,
{
    fn from(f: F) -> Self {
        Payload::Exec(Box::new(f))
    }
}

#[derive(Default)]
pub struct EventLoop<A: Actor> {
    pub ctx: Context<A>,
}

impl<A: Actor> EventLoop<A> {
    pub async fn start(mut self, actor: A) -> ActorResult<Addr<A>> {
        self.spawn(actor).await.unwrap();
        Ok(Addr {
            ctx: Arc::new(self.ctx),
        })
    }

    fn async_loop(
        mut actor: A,
        mut rx: mpsc::Receiver<Payload<A>>,
    ) -> impl Future<Output = ActorResult<()>> {
        async move {
            actor.started().await?;
            eprintln!("waiting for events");
            while let Some(payload) = rx.next().await {
                match payload {
                    Payload::Exec(exec) => {
                        eprintln!("received Exec");
                        exec(&mut actor).await?;
                    }
                    Payload::Stop => break,
                }
            }
            actor.stopped().await?;
            Ok(())
        }
    }

    // TODO: return a handle to the spawned task for cancellation
    async fn spawn(&mut self, actor: A) -> ActorResult<()> {
        let Some(rx) = self.ctx.take_rx() else {
            eprintln!("Cannot spawn context");
            return Err(SpawnError);
        };
        tokio::spawn(async { Self::async_loop(actor, rx).await.unwrap() });
        Ok(())
    }
}
