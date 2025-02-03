#![cfg(feature = "tokio")]
use hannibal::prelude::*;

struct MyActor(&'static str);

#[message]
struct Greet(&'static str);

#[message(response = i32)]
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

#[tokio::main]
async fn main() {
    let mut addr = MyActor("Caesar").spawn();

    // addressing by the concrete type of the actor
    {
        // send a message without a response
        addr.send(Greet("Hannibal")).await.unwrap();

        // expecting a response
        let addition = addr.call(Add(1, 2)).await;

        println!("The Actor Calculated: {:?}", addition);
    }

    // addressing by the type of the message only
    {
        let sender = addr.sender::<Greet>();
        let caller = addr.caller::<Add>();

        // send a message without a response
        sender.send(Greet("Hannibal")).await.unwrap();

        // expecting a response
        let addition = caller.call(Add(1, 2)).await;
        println!("The Actor Calculated: {:?}", addition);
    }

    println!("{:#?}", addr.stop());
}
