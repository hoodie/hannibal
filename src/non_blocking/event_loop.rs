use super::{addr, Actor, Addr, Context};
use crate::{error::ActorError::SpawnError, ActorResult};

use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc};

use futures::{
    channel::mpsc::{self},
    StreamExt,
};

type Execution = dyn for<'a> FnOnce(&'a mut dyn Actor) -> Pin<Box<dyn Future<Output = ActorResult<()>> + Send>>
    + Send
    + 'static;

pub enum Payload {
    Exec(Box<Execution>),
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

#[derive(Default)]
pub struct EventLoop {
    pub ctx: Context,
}

impl EventLoop {
    pub async fn start<A>(mut self, actor: A) -> ActorResult<Addr<A>>
    where
        A: Actor + Send + Sync + 'static,
    {
        self.spawn(actor).await.unwrap();
        Ok(Addr {
            ctx: Arc::new(self.ctx),
            marker: PhantomData::<A>,
        })
    }

    pub(crate) fn async_loop<A>(
        mut actor: A,
        mut rx: mpsc::Receiver<Payload>,
    ) -> impl Future<Output = ActorResult<()>>
    where
        A: Actor + Send + Sync + 'static,
    {
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

    pub async fn spawn<A>(&mut self, actor: A) -> ActorResult<()>
    where
        A: Actor + Send + Sync + 'static,
    {
        let Some(rx) = self.ctx.take_rx() else {
            eprintln!("Cannot spawn context");
            return Err(SpawnError);
        };
        tokio::spawn(async { Self::async_loop(actor, rx).await.unwrap() });
        todo!();
    }
}
