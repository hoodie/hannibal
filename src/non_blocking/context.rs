#![allow(unused_imports)]
use std::sync::{Arc, Mutex}; // TODO: use async_lock::Mutex;

use async_lock::RwLock;
// use async_lock::Mutex;
use futures::{channel::mpsc, SinkExt};

use super::{AsyncHandler, Payload};
use crate::error::ActorError::WriteError;
use crate::ActorResult;

pub struct AsyncContext {
    pub tx: Arc<mpsc::Sender<Payload>>,
    rx: Mutex<Option<mpsc::Receiver<Payload>>>,
}

impl Default for AsyncContext {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel::<Payload>(1000);
        AsyncContext {
            tx: Arc::new(tx),
            rx: Mutex::new(Some(rx)),
        }
    }
}

impl AsyncContext {
    pub fn take_rx(&mut self) -> Option<mpsc::Receiver<Payload>> {
        self.rx.lock().ok().and_then(|mut orx| orx.take())
    }

    pub async fn send<M, H>(&self, msg: M, handler: Arc<RwLock<H>>) -> ActorResult<()>
    where
        H: AsyncHandler<M> + 'static,
        M: Send + 'static,
    {
        let handler = handler.clone();
        eprintln!("sending closoure");
        let mut tx = Arc::unwrap_or_clone(self.tx.clone());
        tx.send(Payload::Exec(Box::new(move || {
            eprintln!("sent closoure");
            Box::pin(async move {
                eprintln!("awaited sent closoure");
                handler.write().await.handle(msg).await?;
                Ok(())
            })
        })))
        .await?;
        Ok(())
    }

    // TODO: add oneshot to notify Addrs
    // TODO: mark self as stopped for loop
    pub async fn stop(&self) -> ActorResult<()> {
        let mut tx = Arc::unwrap_or_clone(self.tx.clone());
        Ok(tx.send(Payload::Stop).await?)
    }
}
