mod addr;
mod context;
mod event_loop;
mod sender;

use crate::ActorResult;

pub use self::{
    addr::Addr,
    context::Context,
    event_loop::{EventLoop, Payload},
    sender::Sender,
};

pub trait Actor: Send + Sync + 'static {
    async fn started(self: &mut Self) -> ActorResult<()>;
    async fn stopped(self: &mut Self) -> ActorResult<()>;
}

// pub trait AsyncHandler<M>: asyncactor {
pub trait Handler<M>: Actor {
    async fn handle(&mut self, msg: M) -> ActorResult<()>;
}
