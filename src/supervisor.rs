use crate::{error::Result, lifecycle::LifeCycle, Actor, Addr};

/// Actor supervisor
///
/// Supervisor gives the actor the ability to restart after failure.
/// When the actor fails, recreate a new actor instance and replace it.
pub struct Supervisor;

impl Supervisor {
    pub async fn start<A, F>(f: F) -> Result<Addr<A>>
    where
        A: Actor,
        F: Fn() -> A + Send + 'static,
    {
        LifeCycle::new().start_supervised(f).await
    }
}
