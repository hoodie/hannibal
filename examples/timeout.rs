use std::time::Duration;

use hannibal::{error::ActorError, prelude::*, RestartableActor};

#[cfg(feature = "async-std")]
use async_std::task::sleep;
#[cfg(feature = "tokio")]
use tokio::time::sleep;

#[derive(Debug, Default)]
struct SleepyActor(u8);

#[message]
struct Sleep(Duration);

impl Actor for SleepyActor {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult<()> {
        println!("[ SleepyActor {} ] started", self.0);
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("[ SleepyActor {} ] stopped", self.0);
    }
}

impl Handler<Sleep> for SleepyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Sleep) {
        println!("[ SleepyActor {} ] is resting for {:?}", self.0, msg.0);
        sleep(msg.0).await;
        println!("[ SleepyActor {} ] woke up after {:?}", self.0, msg.0);
    }
}

// TODO: can we encode the restart strategy in an associated type or as a trait function?
impl RestartableActor for SleepyActor {}

#[tokio::main]
async fn main() {
    // normal case, tasks take long
    println!("SleepyActor 0 will take 1 second to complete");
    let mut addr = hannibal::build(SleepyActor(0))
        .bounded(1)
        .recreate_from_default()
        .spawn();
    addr.call(Sleep(Duration::from_millis(100))).await.unwrap();
    assert!(addr.stop().is_ok());

    // timeout and continue
    println!("SleepyActor 1 will be canceled after 1 second");
    println!("SleepyActor 1 still accepts messages after being canceled");
    let mut addr = hannibal::build(SleepyActor(1))
        .bounded(1)
        .timeout(Duration::from_millis(100))
        .fail_on_timeout(false)
        .recreate_from_default()
        .spawn_owning();
    assert!(addr
        .as_ref()
        .call(Sleep(Duration::from_secs(0)))
        .await
        .is_ok());
    assert!(matches!(
        addr.call(Sleep(Duration::from_secs(4))).await.unwrap_err(),
        ActorError::Canceled(_)
    ));
    assert!(addr.call(Sleep(Duration::from_secs(0))).await.is_ok());
    eprintln!("SleepyActor 2 is still alive, stopping");
    assert!(addr.to_addr().stop().is_ok());
    assert!(addr.join().await.is_some());

    // timeout and fail
    println!("SleepyActor 2 will be canceled after 1 second");
    let mut addr = hannibal::build(SleepyActor(2))
        .bounded(1)
        .timeout(Duration::from_millis(100))
        .fail_on_timeout(true)
        .recreate_from_default()
        .spawn_owning();
    assert!(addr
        .as_ref()
        .call(Sleep(Duration::from_secs(60)))
        .await
        .is_err());
    println!("SleepyActor 2 nolonger accepts messages after being canceled");
    assert!(addr
        .as_ref()
        .call(Sleep(Duration::from_secs(0)))
        .await
        .is_err());
    assert!(addr.join().await.is_none());
    assert!(addr.to_addr().stop().is_err());
}
