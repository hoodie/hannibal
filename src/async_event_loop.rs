#![allow(unused_imports)]
use crate::{Actor, ActorResult, Addr, Context};
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

#[derive(Default)]
pub struct AsyncEventLoop {
    pub ctx: Context,
}

impl AsyncEventLoop {
    pub fn start<A>(mut self, actor: A) -> Addr<A>
    where
        A: Actor + Send + Sync + 'static,
    {
        let actor = Arc::new(RwLock::new(actor));
        self.spawn(actor.clone()).unwrap();
        Addr {
            ctx: Arc::new(self.ctx),
            actor,
        }
    }

    pub(crate) fn async_loop(
        // actor: Arc<dyn Actor + Send + Sync + 'static>,
        mut rx: mpsc::Receiver<Payload>,
        // mut rx: Arc<async_lock::Mutex<mpsc::Receiver<Payload>>>,
        // mut rx: Pin<Box<mpsc::Receiver<Payload>>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
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
                        exec(/*&actor*/).await
                    }
                    Payload::Stop => break,
                }
            }
            // actor.stopped();
        }) // as Pin<Box<dyn Future<Output = ()>>
    }

    #[allow(unused)]
    pub fn spawn(&mut self, actor: Arc<dyn Actor + Send + Sync + 'static>) -> Result<(), String> {
        let Some(rx) = self.ctx.take_rx() else {
            eprintln!("Cannot spawn context");
            return Err("Cannot spawn context".to_string());
        };

        let mut pool = futures_executor::LocalPool::new();
        let spawner = pool.spawner();
        eprintln!("pool started");
        spawner.spawn_local(Self::async_loop(/*actor, */ rx));
        pool.run();

        eprintln!("pool ended");
        // std::thread::spawn((Self::async_loop(actor, rx)).unwrap());
        Ok(())
    }
}
