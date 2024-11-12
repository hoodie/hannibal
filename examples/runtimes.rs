#![cfg_attr(
    all(feature = "tokio", feature = "async-std"),
    allow(dead_code, unused)
)]
use minibal::prelude::*;

struct MyActor(&'static str);

struct Greet(&'static str);
impl Message for Greet {
    type Result = ();
}

struct Add(i32, i32);
impl Message for Add {
    type Result = i32;
}

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

#[cfg(any(
    all(feature = "tokio", not(feature = "async-std")),
    all(not(feature = "tokio"), feature = "async-std")
))]
#[cfg_attr(feature = "tokio", tokio::main)]
#[cfg_attr(feature = "async-std", async_std::main)]
async fn main() {
    let mut addr = MyActor("Caesar").spawn();
    addr.send(Greet("Cornelius")).unwrap();
    let addition = addr.call(Add(1, 2)).await;

    println!("The Actor Calculated: {:?}", addition);
    println!("{:#?}", addr.stop());
}

#[cfg(all(feature = "tokio", feature = "async-std"))]
fn main() {}
