#![allow(unused_imports)]
use crate::{
    non_blocking::{AsyncAddr, AsyncContext},
    ActorResult, AsyncActor,
};
use std::sync::Arc;
use std::{future::Future, pin::Pin};

use async_lock::RwLock;
use futures::{
    channel::mpsc::{self, Receiver, Sender, UnboundedReceiver, UnboundedSender},
    FutureExt, StreamExt,
};

use futures::task::LocalSpawnExt;

pub enum Payload {
    Exec(
        Box<
            dyn FnOnce(
                    // &'a (dyn Actor + Send + Sync),
                    // &'a mut Context,
                )
                    -> Pin<Box<dyn Future<Output = ActorResult<()>> + Send /*+ 'a*/>>
                + Send
                + 'static,
        >,
    ),
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
        // actor: Arc<dyn Actor + Send + Sync + 'static>,
        mut rx: mpsc::Receiver<Payload>,
        // mut rx: Arc<async_lock::Mutex<mpsc::Receiver<Payload>>>,
        // mut rx: Pin<Box<mpsc::Receiver<Payload>>>,
    ) -> impl Future<Output = Result<(), ()>> {
        // let mut actor = actor.clone();

        // let (_, mut rx) = futures::channel::mpsc::channel::<Payload>(5);
        // use futures::stream::{self, StreamExt};

        // let mut rx = Box::pin(futures::channel::mpsc::channel::<Payload>(5).1);
        // let mut rx = futures::channel::mpsc::channel::<Payload>(5).1;

        // let rx = rx.lock().await;

        Box::pin(async move {
            // actor.started();
            eprintln!("waiting for events");
            while let Some(payload) = rx.next().await {
                match payload {
                    Payload::Exec(exec) => {
                        eprintln!("received Exec");
                        exec(/*&actor*/).await;
                    }
                    Payload::Stop => break,
                }
            }
            Ok(())
            // actor.stopped();
        }) // as Pin<Box<dyn Future<Output = ()>>
    }

    #[allow(unused)]
    pub fn spawn(&mut self, actor: BoxedAsyncActor) -> Result<(), String> {
        let Some(rx) = self.ctx.take_rx() else {
            eprintln!("Cannot spawn context");
            return Err("Cannot spawn context".to_string());
        };

        todo!();
        // std::thread::spawn((Self::async_loop(actor, rx)).unwrap());
        Ok(())
    }
}
