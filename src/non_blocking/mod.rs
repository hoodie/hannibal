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
    async fn started(self: &mut Self) -> ActorResult<()>;
    async fn stopped(self: &mut Self) -> ActorResult<()>;
}

// pub trait AsyncHandler<M>: asyncactor {
pub trait AsyncHandler<M> {
    async fn handle(&mut self, msg: M) -> ActorResult<()>;
}

pub type ActorOuter<A> = std::sync::Arc<async_lock::RwLock<A>>;
