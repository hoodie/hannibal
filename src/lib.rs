mod addr;
mod context;
mod event_loop;
mod sender;
mod channel;

mod error;

pub use addr::Addr;
pub use context::Context;
pub use event_loop::EventLoop;
pub use sender::Sender;

pub type ActorResult<T> = Result<T, error::ActorError>;

pub mod non_blocking;

pub trait Actor {
    fn started(&mut self) -> ActorResult<()>;
    fn stopped(&mut self) -> ActorResult<()>;
}

pub trait Handler<M>: Actor + Send + Sync {
    fn handle(&mut self, msg: M);
}
