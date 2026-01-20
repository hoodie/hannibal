use hannibal::{messages::Ping, prelude::*};

struct MyActor;

#[message(response = i32)]
struct Add(i32, i32);

impl Actor for MyActor {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult<()> {
        println!("[Actor] started");
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("[Actor] stopped");
    }
}

impl Handler<Add> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Add) -> i32 {
        msg.0 + msg.1
    }
}

#[hannibal::main]
async fn main() {
    env_logger::init();
    // let addr = MyActor.spawn_owning();
    let mut addr = MyActor.spawn();

    addr.ping().await.unwrap();

    let addition = addr.call(Add(1, 2)).await;
    println!("The Actor Calculated: {addition:?}");

    let addition = addr.call(Add(3, 4)).await;
    println!("The Actor Calculated: {addition:?}");

    let addition = addr.call(Add(5, 6)).await;
    println!("The Actor Calculated: {addition:?}");

    let pong = addr.call(Ping::default()).await.unwrap();
    println!("The Actor Ponged: {:?}", pong.0);

    addr.stop().unwrap();
    // addr.consume().await.unwrap();
}
