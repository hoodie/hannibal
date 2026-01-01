use hannibal::prelude::*;

struct MyActor(&'static str);

#[message]
struct Greet(&'static str);

#[message]
struct Stop;

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

impl Handler<Stop> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _: Stop) {
        ctx.stop().unwrap();
    }
}

#[hannibal::main]
async fn main() {
    color_backtrace::install();
    env_logger::init();
    let addr = MyActor("Caesar").spawn();

    // send a message without a response
    addr.send(Greet("Hannibal")).await.unwrap();

    // Stopping the actor
    addr.send(Stop).await.unwrap();
    println!("waiting for stop: {:?}", addr.clone().await);

    println!(
        "trying to greet again: {:?}",
        addr.try_send(Greet("Hannibal")).unwrap_err()
    );
    println!("trying to ping: {:?}", addr.ping().await.unwrap_err());

    let sender = addr.sender::<Greet>();

    println!(
        "trying to greet via fresh sender: {:?}",
        sender.send(Greet("Hannibal")).await.unwrap_err()
    );
}
