use std::sync::{mpsc, Arc, Mutex, RwLock};

use crate::{error::ActorError::WriteError, event_loop::Payload, ActorResult, Handler};

pub struct Context {
    pub tx: Arc<mpsc::Sender<Payload>>,
    rx: Mutex<Option<mpsc::Receiver<Payload>>>,
}

impl Default for Context {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<Payload>();
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

    pub fn send<M, H>(&self, msg: M, handler: Arc<RwLock<H>>) -> ActorResult<()>
    where
        H: Handler<M> + 'static,
        M: Send + Sync + 'static,
    {
        let handler = handler.clone();
        self.tx.send(Payload::from(move || {
            let mut handler = handler.write().map_err(|_| WriteError)?;
            handler.handle(msg);
            Ok(())
        }))?;
        Ok(())
    }

    // TODO: add oneshot to notify Addrs
    // TODO: mark self as stopped for loop
    pub fn stop(&self) -> ActorResult<()> {
        Ok(self.tx.send(Payload::Stop)?)
    }
}
