use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use futures::{channel::mpsc, SinkExt};

use super::Payload;
use crate::ActorResult;

pub struct Context {
    pub tx: Arc<mpsc::Sender<Payload>>,
    rx: Mutex<Option<mpsc::Receiver<Payload>>>,
}

impl Default for Context {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel::<Payload>(1000);
        Context {
            tx: Arc::new(tx),
            rx: Mutex::new(Some(rx)),
        }
    }
}

impl Context {
    pub fn take_rx(&mut self) -> Option<mpsc::Receiver<Payload>> {
        self.rx.lock().ok().and_then(|mut orx| orx.take())
    }

    pub async fn send<M, H>(&self, msg: M) -> ActorResult<()>
    where
        M: Send + 'static,
    {
        // let handler = handler.clone();
        eprintln!("sending closoure");
        let mut tx = Arc::unwrap_or_clone(self.tx.clone());
        tx.send(Payload::from(move |handler| {
            eprintln!("sent closoure");
            Box::pin(async move {
                eprintln!("awaited sent closoure");
                handler.handle(msg).await?;
                Ok(())
            }) as Pin<Box<dyn Future<Output = ActorResult<()>> + Send>>
        }))
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
