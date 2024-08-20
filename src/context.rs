use std::sync::{mpsc, Arc, Mutex};

use crate::{event_loop::Payload, Handler};

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

    pub fn send<M, H>(&self, msg: M, handler: Arc<H>)
    where
        H: Handler<M> + 'static,
        M: Send + 'static,
    {
        let handler = handler.clone();
        self.tx
            .send(Payload::Exec(Box::new(move || {
                let handler = handler.clone();
                handler.handle(msg);
            })))
            .unwrap()
    }

    // TODO: add oneshot to notify Addrs
    // TODO: mark self as stopped for loop
    pub fn stop(&self) {
        self.tx.send(Payload::Stop).unwrap();
    }
}
