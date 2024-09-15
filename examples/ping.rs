use hannibal::*;

/// Define `Ping` message
#[message(result = usize)]
struct Ping(usize);

/// Actor
struct MyActor {
    count: usize,
}

/// Declare actor and its context
impl Actor for MyActor {}

/// Handler for `Ping` message
impl Handler<Ping> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Ping) -> usize {
        eprintln!("PING: {}", <Ping as Message>::NAME);
        self.count += msg.0;
        self.count
    }
}

#[hannibal::main]
async fn main() -> Result<()> {
    // start new actor
    let addr = MyActor { count: 10 }.start().await?;
    let caller: &dyn std::fmt::Debug = &Some(addr.caller());
    println!("caller: {caller:?}");

    // send message and get future for result
    let res = addr.call(Ping(10)).await?;

    println!("RESULT: {}", res == 20);

    Ok(())
}
