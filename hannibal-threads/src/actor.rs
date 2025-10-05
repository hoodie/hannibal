pub use crate::ActorResult;
pub trait Actor {
    fn started(&mut self) -> ActorResult<()>;
    fn stopped(&mut self) -> ActorResult<()>;
}
