use std::{
    marker::PhantomData,
    sync::{mpsc, Arc, Mutex, Weak},
};

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

pub struct Addr<A: Actor> {
    ctx: Arc<Mutex<Context>>,
    actor: Arc<A>,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Addr {
            ctx: self.ctx.clone(),
            actor: self.actor.clone(),
        }
    }
}

impl<A: Actor> Addr<A> {
    pub fn send<M>(&self, msg: M)
    where
        A: Handler<M> + 'static,
        M: Send + 'static,
    {
        self.ctx.lock().unwrap().send(msg, self.actor.clone());
    }

    pub fn downgrade(&self) -> WeakAddr<A> {
        WeakAddr {
            ctx: Arc::downgrade(&self.ctx),
            actor: Arc::downgrade(&self.actor),
        }
    }

    pub fn sender<M>(&self) -> Sender<M>
    where
        A: Handler<M> + 'static,
        M: Send + 'static,
    {
        (*self).clone().into()
    }
}

pub struct WeakAddr<A: Actor> {
    ctx: Weak<Mutex<Context>>,
    actor: Weak<A>,
}

impl<A: Actor> WeakAddr<A> {
    pub fn upgrade(&self) -> Option<Addr<A>> {
        Some(Addr {
            ctx: self.ctx.upgrade()?,
            actor: self.actor.upgrade()?,
        })
    }

    pub fn try_send<M>(&self, msg: M)
    where
        A: Handler<M> + 'static,
        M: Send + 'static,
    {
        if let Some(addr) = self.upgrade() {
            addr.send(msg)
        }
    }
}

pub struct LifeCycle {
    pub ctx: Context,
}

pub trait Actor {
    fn started(&self);
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

    pub fn start<A>(mut self, actor: Arc<A>) -> Addr<A>
    where
        A: Actor + Send + Sync + 'static,
    {
        let LifeCycle { ref mut ctx } = self;
        ctx.spawn(actor.clone());
        Addr {
            ctx: Arc::new(Mutex::new(self.ctx)),
            actor,
        }
    }
}

pub trait Handler<M>: Actor + Send + Sync {
    fn handle(&self, msg: M);
}

pub struct Sender<M> {
    tx: Arc<mpsc::Sender<Payload>>,
    actor: Arc<dyn Handler<M>>,
    marker: PhantomData<M>,
}

impl<M, A> From<Addr<A>> for Sender<M>
where
    A: Handler<M> + 'static,
{
    fn from(Addr { ctx, actor }: Addr<A>) -> Self {
        Sender {
            tx: ctx.lock().unwrap().tx.clone(),
            actor: actor.clone(),
            marker: PhantomData,
        }
    }
}

impl<M> Sender<M> {
    pub fn send(&self, msg: M)
    where
        M: Send + 'static,
    {
        let actor = self.actor.clone();
        self.tx.send(Box::new(move || actor.handle(msg))).unwrap()
    }

    pub fn downgrade(&self) -> WeakSender<M> {
        WeakSender {
            tx: Arc::downgrade(&self.tx),
            actor: Arc::downgrade(&self.actor),
            marker: PhantomData,
        }
    }
}

pub struct WeakSender<M> {
    tx: Weak<mpsc::Sender<Payload>>,
    actor: Weak<dyn Handler<M>>,
    marker: PhantomData<M>,
}

impl<M> WeakSender<M> {
    pub fn try_send(&self, msg: M) -> bool
    where
        M: Send + 'static,
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
