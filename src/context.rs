use event_loop::Payload;

use super::*;

pub struct Context {
    pub(crate) tx: Arc<mpsc::Sender<Payload>>,
    pub(crate) rx: Arc<Mutex<Option<mpsc::Receiver<Payload>>>>,
}

impl Context {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<Payload>();
        Context {
            tx: Arc::new(tx),
            rx: Arc::new(Mutex::new(Some(rx))),
        }
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
