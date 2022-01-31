/// Actor
struct MyActor;

/// Declare actor and its context
#[async_trait::async_trait]
impl hannibal::Actor for MyActor {
    async fn stopped(&mut self, _ctx: &mut hannibal::Context<Self>) {
        println!("stopped");
    }
}

#[test]
fn stop_addr() {
    async fn main() -> hannibal::Result<()> {
        let mut addr = hannibal::Actor::start(MyActor).await?;
        let addr2 = addr.clone();

        assert!(!addr.stopped(), "expected addr not to be stopped");
        assert!(!addr2.stopped(), "expected addr2 not to be stopped");

        addr.stop(None).unwrap();
        assert!(!addr.stopped());
        assert!(!addr2.stopped());
        addr.wait_for_stop().await;

        assert!(addr2.stopped(), "expected addr2 to be stopped");
        let addr3 = addr2.clone();
        assert!(addr3.stopped(), "expected addr3 to be stopped");

        Ok(())
    }

    hannibal::block_on(main()).unwrap();
}
