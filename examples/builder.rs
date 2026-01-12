use futures::stream;
use hannibal::{RestartableActor, prelude::*};

#[derive(Default)]
struct MyActor(&'static str);

#[message]
struct Greet(&'static str);

#[message(response= i32)]
struct Add(i32, i32);

impl Actor for MyActor {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult<()> {
        println!("[Actor {}] started", self.0);
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("[Actor {}] stopped", self.0);
    }
}

impl Handler<Greet> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Greet) {
        println!(
            "[Actor {me}] Hello {you}, my name is {me}",
            me = self.0,
            you = msg.0,
        );
    }
}

impl Handler<Add> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Add) -> i32 {
        msg.0 + msg.1
    }
}

impl StreamHandler<i32> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
        println!("[Actor {}] Received: {msg}", self.0);
    }
}

impl Service for MyActor {}

async fn send_greet_and_stop(mut addr: Addr<MyActor>) {
    addr.send(Greet("Cornelius")).await.unwrap();
    let addition = addr.call(Add(1, 2)).await;

    println!("The Actor Calculated: {addition:?}");
    println!("{:#?}", addr.stop());
}

// TODO: can we encode the restart strategy in an associated type or as a trait function?
impl RestartableActor for MyActor {}

#[hannibal::main]
async fn main() {
    // Basic spawn with bounded channel and restart strategy
    let addr = hannibal::setup_actor(MyActor("Caesar"))
        .bounded(6)
        .recreate_from_default()
        .spawn();
    send_greet_and_stop(addr).await;

    // Register as a service
    hannibal::setup_actor(MyActor("Caesar"))
        .bounded(6)
        .recreate_from_default()
        .register()
        .await
        .unwrap();

    let addr = MyActor("Caesar").spawn();
    send_greet_and_stop(addr).await;

    let addr = MyActor("Caesar")
        .setup_actor()
        .bounded(6)
        .recreate_from_default()
        .spawn();
    send_greet_and_stop(addr).await;

    // Stream with on_stream first, then bounded
    let addr = hannibal::setup_actor(MyActor("Caesar"))
        .on_stream(stream::iter(17..19))
        .bounded(10)
        .spawn();
    send_greet_and_stop(addr).await;

    // Stream with bounded first, then on_stream
    let addr = hannibal::setup_actor(MyActor("Caesar"))
        .bounded(10)
        .on_stream(stream::iter(17..19))
        .spawn();
    send_greet_and_stop(addr).await;

    // Simple unbounded spawn
    let addr = MyActor("Caesar").setup_actor().unbounded().spawn();
    send_greet_and_stop(addr).await;

    // Direct spawn on actor (uses defaults)
    let addr = MyActor("Caesar").spawn();
    send_greet_and_stop(addr).await;
}
