use std::sync::{mpsc, Arc, Mutex, Weak};

type Payload = Box<dyn FnOnce() + Send + 'static>;

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
        H: Handler<M>,
        H: 'static,
        M: Send,
        M: 'static,
    {
        let handler = handler.clone();
        self.tx
            .send(Box::new(move || {
                let handler = handler.clone();
                handler.handle(msg);
            }))
            .unwrap()
    }

    pub fn spawn(&mut self) {
        let rx = self.rx.lock().unwrap().take().unwrap();
        std::thread::spawn(move || {
            let receiving = rx.iter();
            for payload in receiving {
                payload();
            }
        });
    }
}

#[derive(Clone)]
pub struct Addr<A: Actor> {
    ctx: Arc<Mutex<Context>>,
    actor: Arc<A>,
}

impl<A: Actor> Addr<A> {
    pub fn send<M>(&self, msg: M)
    where
        A: Handler<M>,
        M: Send,
        A: 'static,
        M: 'static,
    {
        self.ctx.lock().unwrap().send(msg, self.actor.clone());
    }

    pub fn sender<M>(&self) -> Sender<M>
    where
        A: Handler<M>,
        M: Send,
        A: 'static,
        M: 'static,
    {
        Sender {
            tx: self.ctx.lock().unwrap().tx.clone(),
            actor: self.actor.clone(),
            marker: std::marker::PhantomData,
        }
    }
}

pub struct LifeCycle {
    pub ctx: Context,
}

pub trait Actor {
    fn start(&self);
}

impl Default for LifeCycle {
    fn default() -> Self {
        Self::new()
    }
}

impl LifeCycle {
    pub fn new() -> Self {
        LifeCycle {
            ctx: Context::new(),
        }
    }

    pub fn start<A: Actor>(mut self, actor: A) -> Addr<A> {
        let LifeCycle { ref mut ctx } = self;
        ctx.spawn();
        Addr {
            ctx: Arc::new(Mutex::new(self.ctx)),
            actor: Arc::new(actor),
        }
    }
}

pub trait Handler<M>: Actor + Send + Sync {
    fn handle(&self, msg: M);
}

pub struct Sender<M> {
    tx: Arc<mpsc::Sender<Payload>>,
    actor: Arc<dyn Handler<M>>,
    marker: std::marker::PhantomData<M>,
}

impl<M> Sender<M> {
    pub fn send(&self, msg: M)
    where
        M: Send,
        M: 'static,
    {
        let actor = self.actor.clone();
        self.tx.send(Box::new(move || actor.handle(msg))).unwrap()
    }

    pub fn downgrade(&self) -> WeakSender<M> {
        WeakSender {
            tx: Arc::downgrade(&self.tx),
            actor: Arc::downgrade(&self.actor),
            marker: std::marker::PhantomData,
        }
    }
}

pub struct WeakSender<M> {
    tx: Weak<mpsc::Sender<Payload>>,
    actor: Weak<dyn Handler<M>>,
    marker: std::marker::PhantomData<M>,
}

impl<M> WeakSender<M> {
    pub fn try_send(&self, msg: M) -> bool
    where
        M: Send,
        M: 'static,
    {
        if let Some((tx, actor)) = self.tx.upgrade().zip(self.actor.upgrade()) {
            tx.send(Box::new(move || actor.handle(msg))).unwrap();
            true
        } else {
            eprintln!("Actor is dead");
            false
        }
    }
}
