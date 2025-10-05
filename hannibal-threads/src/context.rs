use std::sync::{Arc, Mutex, RwLock, mpsc};

use crate::{ActorResult, error::ActorError::WriteError, event_loop::Payload, handler::Handler};

pub struct Context<A> {
    pub tx: Arc<mpsc::Sender<Payload>>,
    rx: Mutex<Option<mpsc::Receiver<Payload>>>,
    phantom: std::marker::PhantomData<A>,
}

impl<A> Default for Context<A> {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<Payload>();
        Context {
            tx: Arc::new(tx),
            rx: Mutex::new(Some(rx)),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<A> Context<A> {
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
