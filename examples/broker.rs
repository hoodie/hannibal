use hannibal::{Broker, prelude::*, subscribe_to};

#[derive(Clone, Debug, Message)]
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

#[hannibal::main]
async fn main() {
    env_logger::init();
    let mut subscriber1 = Subscribing::default().spawn_owning();
    subscriber1.ping().await.unwrap();

    let mut subscriber2 = Subscribing::default().spawn_owning();
    subscriber2.ping().await.unwrap();

    let external_receiver = subscribe_to::<Topic1>(10).await.unwrap();

    let broker = Broker::from_registry().await;

    broker.publish(Topic1(42)).await.unwrap();
    broker.publish(Topic1(23)).await.unwrap();

    assert_eq!(subscriber1.join().await, Some(Subscribing(vec![42, 23])));
    assert_eq!(subscriber2.join().await, Some(Subscribing(vec![42, 23])));

    println!("both subscribers received all messages");

    let msg1 = external_receiver.recv().await.unwrap();
    let msg2 = external_receiver.recv().await.unwrap();
    println!("external received: {msg1:?} and {msg2:?}");
}
