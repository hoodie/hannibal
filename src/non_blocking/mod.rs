use std::pin::Pin;

mod addr;
mod context;
mod event_loop;
mod sender;

use crate::ActorResult;

pub use self::addr::AsyncAddr;
pub use self::context::AsyncContext;
pub use self::event_loop::{AsyncEventLoop, Payload};
pub use self::sender::AsyncSender;

pub trait AsyncActor: Send + Sync + 'static {
    fn started(&mut self) -> Pin<Box<dyn std::future::Future<Output = ActorResult<()>> + Send>>;
    fn stopped(&mut self) -> Pin<Box<dyn std::future::Future<Output = ActorResult<()>> + Send>>;
}

pub trait AsyncHandler<M>: AsyncActor + Send + Sync {
    fn handle(&self, msg: M) -> Pin<Box<dyn std::future::Future<Output = ActorResult<()>> + Send>>;
}
