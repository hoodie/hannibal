use minibal::{prelude::*, Broker};
use std::time::Duration;

#[derive(Clone)]
struct Topic1(u32);
impl Message for Topic1 {
    type Result = ();
}

struct GetValue;
impl Message for GetValue {
    type Result = Vec<u32>;
}

#[derive(Default)]
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

impl Handler<GetValue> for Subscribing {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: GetValue) -> Vec<u32> {
        self.0.clone()
    }
}

#[tokio::main]
async fn main() -> DynResult<()> {
    let subscriber1 = Subscribing::default().spawn();
    let subscriber2 = Subscribing::default().spawn();

    Broker::from_registry()
        .await
        .publish(Topic1(42))
        .await
        .unwrap();
    Broker::publish(Topic1(23)).await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await; // Wait for the messages

    assert_eq!(subscriber1.call(GetValue).await?, vec![42, 23]);
    assert_eq!(subscriber2.call(GetValue).await?, vec![42, 23]);
    Ok(())
}
