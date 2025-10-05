pub use crate::ActorResult;
pub trait Actor: Sized + Send + 'static {
    fn started(&mut self) -> ActorResult<()>;
    fn stopped(&mut self) -> ActorResult<()>;
}
