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
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: Topic1) {
        self.0.push(msg.0);
        log::debug!("received {}", msg.0);
        if self.0.len() == 2 {
            ctx.stop().unwrap()
        }
    }
}

#[tokio::main]
async fn main() {
    let mut subscriber1 = Subscribing::default().spawn_owning();
    subscriber1.ping().await.unwrap();

    let mut subscriber2 = Subscribing::default().spawn_owning();
    subscriber2.ping().await.unwrap();

    let broker = Broker::from_registry().await;
    broker.publish(Topic1(42)).await.unwrap();
    broker.publish(Topic1(23)).await.unwrap();

    assert_eq!(subscriber1.join().await, Some(Subscribing(vec![42, 23])));
    assert_eq!(subscriber2.join().await, Some(Subscribing(vec![42, 23])));
    println!("both subscribers received all messges");
}
