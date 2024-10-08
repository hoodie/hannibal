use crate::{error::Result, lifecycle::LifeCycle, Actor, Addr};

/// Actor supervisor.
///
/// Supervisor gives the actor the ability to restart after failure.
/// When the actor fails, recreate a new actor instance and replace it.
pub struct Supervisor;

impl Supervisor {
    /// Start a supervisor
    ///
    /// # Examples
    ///
    /// ```rust
    /// use hannibal::*;
    /// use std::time::Duration;
    ///
    /// #[message]
    /// struct Die;
    ///
    /// #[message]
    /// struct Add;
    ///
    /// #[message(result = i32)]
    /// struct Get;
    ///
    /// struct MyActor(i32);
    ///
    /// impl Actor for MyActor {}
    ///
    /// impl Handler<Add> for MyActor {
    ///     async fn handle(&mut self, ctx: &mut Context<Self>, _: Add) {
    ///         self.0 += 1;
    ///     }
    /// }
    ///
    /// impl Handler<Get> for MyActor {
    ///     async fn handle(&mut self, ctx: &mut Context<Self>, _: Get) -> i32 {
    ///         self.0
    ///     }
    /// }
    ///
    /// impl Handler<Die> for MyActor {
    ///     async fn handle(&mut self, ctx: &mut Context<Self>, _: Die) {
    ///         ctx.stop(None);
    ///     }
    /// }
    ///
    /// #[hannibal::main]
    /// async fn main() -> Result<()> {
    ///     let mut addr = Supervisor::start(|| MyActor(0)).await?;
    ///
    ///     addr.send(Add)?;
    ///     assert_eq!(addr.call(Get).await?, 1);
    ///
    ///     addr.send(Add)?;
    ///     assert_eq!(addr.call(Get).await?, 2);
    ///
    ///     addr.send(Die)?;
    ///     sleep(Duration::from_secs(1)).await; // Wait for actor restart
    ///
    ///     assert_eq!(addr.call(Get).await?, 0);
    ///     Ok(())
    /// }
    /// ```
    pub async fn start<A, F>(f: F) -> Result<Addr<A>>
    where
        A: Actor,
        F: Fn() -> A + Send + 'static,
    {
        LifeCycle::new().start_supervised(f).await
    }
}
