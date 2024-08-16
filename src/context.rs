use super::*;

pub(super) type Payload = Box<dyn FnOnce() + Send + 'static>;

pub struct Context {
    pub tx: Arc<mpsc::Sender<Payload>>,
    pub rx: Arc<Mutex<Option<mpsc::Receiver<Payload>>>>,
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
            .send(Box::new(move || {
                let handler = handler.clone();
                handler.handle(msg);
            }))
            .unwrap()
    }

    pub fn spawn(&mut self, actor: Arc<dyn Actor + Send + Sync>) {
        let Some(rx) = self.rx.lock().ok().and_then(|mut orx| orx.take()) else {
            eprintln!("Cannot spawn context");
            return;
        };

        std::thread::spawn(move || {
            actor.started();
            let receiving = rx.iter();
            for payload in receiving {
                payload();
            }
        });
    }
}
