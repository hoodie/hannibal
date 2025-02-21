use futures::future::join;
use hannibal::{Broker, prelude::*};

#[derive(Clone, Message)]
struct Topic1(u32);

#[derive(Debug, Default, PartialEq)]
struct Subscribing(Vec<u32>);

impl Actor for Subscribing {
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        ctx.subscribe::<Topic1>().await?;
        Ok(())
    }
}

impl Handler<Topic1> for Subscribing {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Topic1) {
        self.0.push(msg.0);
    }
}

#[tokio::main]
async fn main() {
    let subscriber1 = Subscribing::default().spawn_owning();
    let subscriber2 = Subscribing::default().spawn_owning();

    // avoid race condition in example
    let ping_both = || join(subscriber1.ping(), subscriber2.ping());

    let _ = ping_both().await;

    let broker = Broker::from_registry().await;
    broker.publish(Topic1(42)).await.unwrap();
    broker.publish(Topic1(23)).await.unwrap();

    // avoid race condition in example
    let _ = ping_both().await;

    assert_eq!(subscriber1.consume().await, Ok(Subscribing(vec![42, 23])));
    assert_eq!(subscriber2.consume().await, Ok(Subscribing(vec![42, 23])));
    println!("both subscribers received all messges");
}
