use hannibal::{message, Handler};

/// Actor
struct MyActor;

/// Declare actor and its context
impl hannibal::Actor for MyActor {
    async fn stopped(&mut self, _ctx: &mut hannibal::Context<Self>) {
        println!("stopped");
    }
}

#[message]
struct Ping;
impl Handler<Ping> for MyActor {
    async fn handle(&mut self, _ctx: &mut hannibal::Context<Self>, _msg: Ping) {
        println!("ping");
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

        Ok(())
    }

    hannibal::block_on(main()).unwrap();
}

#[test]
fn drop_addr() {
    async fn main() -> hannibal::Result<()> {
        let addr = hannibal::Actor::start(MyActor).await?;
        let addr2 = addr.clone();
        let addr3 = addr.downgrade();
        let caller = addr.weak_caller::<Ping>();

        assert!(!addr.stopped(), "expected addr not to be stopped");
        assert!(!addr2.stopped(), "expected addr2 not to be stopped");
        assert!(
            !addr3.upgrade().unwrap().stopped(),
            "expected addr3 not to be stopped"
        );
        assert!(caller.upgrade().is_some());

        drop(addr);
        assert!(!addr2.stopped());
        assert!(!addr3.upgrade().unwrap().stopped());
        assert!(caller.upgrade().is_some());

        drop(addr2);
        assert!(!caller.upgrade().is_some());
        assert!(addr3.upgrade().is_none(), "expected addr3 to be stopped");

        Ok(())
    }

    hannibal::block_on(main()).unwrap();
}
