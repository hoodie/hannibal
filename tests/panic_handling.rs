use hannibal::prelude::*;

#[derive(Actor)]
struct StoppableActor;

#[derive(Actor)]
struct PanickingActor;

#[message(response = ())]
struct PanicNow;

#[message(response = String)]
struct Echo(String);

impl Handler<PanicNow> for PanickingActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _: PanicNow) {
        panic!("ooops, but this is not the cause of the failed test!");
    }
}

impl Handler<Echo> for PanickingActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Echo) -> String {
        msg.0
    }
}

/// Helper to wait for actor to be marked as stopped.
///
/// This is necessary because different runtimes have different task cleanup timing:
/// - tokio: Cleanup is synchronous when a task panics, flag is set immediately
/// - async-global-executor: Uses thread pool, cleanup happens on worker thread with slight delay
async fn wait_for_stopped<A: Actor>(addr: &Addr<A>) {
    for i in 0..100 {
        if addr.stopped() {
            eprintln!("saw stopped after {i} sleeps");
            return;
        }
        // Small yield to let runtime complete cleanup
        #[cfg(feature = "tokio_runtime")]
        tokio::task::yield_now().await;

        #[cfg(not(feature = "tokio_runtime"))]
        async_io::Timer::after(std::time::Duration::from_micros(100 + i * 10)).await;
    }
    panic!("Actor never stopped");
}

#[test_log::test(tokio::test)]
async fn running_returns_false_after_panic() {
    let addr = PanickingActor.spawn();

    assert!(addr.running());

    assert_eq!(addr.call(Echo("test".into())).await.unwrap(), "test");

    assert!(addr.running());

    assert!(addr.call(PanicNow).await.is_err());

    wait_for_stopped(&addr).await;
    assert!(addr.ping().await.is_err());
    assert!(addr.stopped());

    assert!(addr.call(Echo("after panic".into())).await.is_err());
}

#[test_log::test(tokio::test)]
async fn running_returns_false_after_normal_stop() {
    let addr = StoppableActor.spawn();
    assert!(addr.running());

    addr.clone().halt().await.unwrap();

    wait_for_stopped(&addr).await;
    assert!(addr.ping().await.is_err());
}

#[test_log::test(tokio::test)]
async fn multiple_addresses_see_consistent_state() {
    let addr1 = PanickingActor.spawn();
    let addr2 = addr1.clone();
    let addr3 = addr1.clone();

    assert!(addr1.running());
    assert!(addr2.running());
    assert!(addr3.running());

    addr1.call(PanicNow).await.unwrap_err();

    wait_for_stopped(&addr1).await;
    assert!(addr1.stopped());
    assert!(addr2.stopped());
    assert!(addr3.stopped());

    assert!(addr1.ping().await.is_err());
    assert!(addr2.ping().await.is_err());
    assert!(addr3.ping().await.is_err());
}

#[test_log::test(tokio::test)]
async fn running_with_fire_and_forget_send() {
    let addr = PanickingActor.spawn();

    assert!(addr.running());

    addr.send(PanicNow).await.unwrap();

    wait_for_stopped(&addr).await;
    assert!(addr.ping().await.is_err());
}
