mod addr;
mod context;
mod event_loop;
mod sender;

mod error;

mod async_addr;
mod async_context;
mod async_event_loop;
mod async_sender;

use std::pin::Pin;

pub use addr::Addr;
pub use context::Context;
pub use event_loop::EventLoop;
pub use sender::Sender;

pub type ActorResult<T> = Result<T, error::ActorError>;
// pub type ActorFuture<T = ()> = dyn std::future::Future<Output = ActorResult<T>> + Send;
// pub type ActorFuture<T = ()> = std::pin::Pin<Box<dyn std::future::Future<Output = ActorResult<T>> + Send>>;

pub mod non_blocking {
    pub use super::async_addr::AsyncAddr;
    pub use super::async_context::AsyncContext;
    pub use super::async_event_loop::AsyncEventLoop;
    pub use super::async_sender::AsyncSender;
}

pub trait Actor {
    fn started(&mut self) -> ActorResult<()>;
    fn stopped(&mut self) -> ActorResult<()>;
}

pub trait Handler<M>: Actor + Send + Sync {
    fn handle(&mut self, msg: M);
}

pub trait AsyncActor: Send + Sync + 'static {
    fn started(&mut self) -> Pin<Box<dyn std::future::Future<Output = ActorResult<()>> + Send>>;
    fn stopped(&mut self) -> Pin<Box<dyn std::future::Future<Output = ActorResult<()>> + Send>>;
}

pub trait AsyncHandler<M>: AsyncActor + Send + Sync {
    fn handle(&self, msg: M) -> Pin<Box<dyn std::future::Future<Output = ActorResult<()>> + Send>>;
}
