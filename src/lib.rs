use std::{
    marker::PhantomData,
    sync::{mpsc, Arc, Mutex, Weak},
};

mod addr;
mod context;
mod event_loop;
mod sender;

pub use addr::Addr;
pub use context::Context;
pub use event_loop::EventLoop;
pub use sender::Sender;

pub trait Actor {
    fn started(&self);
    fn stopped(&self);
}

impl<A: Actor> Actor for Arc<A> {
    fn started(&self) {
        self.as_ref().started()
    }

    fn stopped(&self) {
        self.as_ref().stopped()
    }
}

pub trait Handler<M>: Actor + Send + Sync {
    fn handle(&self, msg: M);
}
