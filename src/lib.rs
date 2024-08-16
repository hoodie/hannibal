use std::{
    marker::PhantomData,
    sync::{mpsc, Arc, Mutex, Weak},
};

mod addr;
mod context;
mod lifecycle;
mod sender;

pub use context::Context;
pub use sender::Sender;
pub use addr::Addr;
pub use lifecycle::LifeCycle;

pub trait Actor {
    fn started(&self);
}

pub trait Handler<M>: Actor + Send + Sync {
    fn handle(&self, msg: M);
}
