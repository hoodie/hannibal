use crate::actor::Actor;

pub use super::error::ActorError;

pub trait Handler<M>: Actor + Send + Sync {
    // TODO: accept context
    fn handle(&mut self, msg: M);
}
