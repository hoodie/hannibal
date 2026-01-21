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
        panic!("ooops");
    }
}

impl Handler<Echo> for PanickingActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Echo) -> String {
        msg.0
    }
}

#[test_log::test(tokio::test)]
async fn running_returns_false_after_panic() {
    let addr = PanickingActor.spawn();

    assert!(addr.running());

    assert_eq!(addr.call(Echo("test".into())).await.unwrap(), "test");

    assert!(addr.running());

    assert!(addr.call(PanicNow).await.is_err());

    assert!(addr.stopped());
    assert!(addr.ping().await.is_err());
    assert!(addr.stopped());

    assert!(addr.call(Echo("after panic".into())).await.is_err());
}

#[test_log::test(tokio::test)]
async fn running_returns_false_after_normal_stop() {
    let addr = StoppableActor.spawn();
    assert!(addr.running());

    addr.clone().halt().await.unwrap();

    assert!(addr.stopped());
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

    assert!(addr.ping().await.is_err());
    assert!(addr.stopped());
}
