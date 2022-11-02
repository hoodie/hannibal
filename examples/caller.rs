use hannibal::*;

/// Define `Ping` message
#[message(result = "usize")]
struct Ping(usize);

#[message]
struct Pow;

/// Actor
struct MyActor {
    count: usize,
}

/// Declare actor and its context
impl Actor for MyActor {}

/// Handler for `Ping` message
#[async_trait::async_trait]
impl Handler<Ping> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Ping) -> usize {
        self.count += msg.0;
        self.count
    }
}

/// Handler for `Ping` message
#[async_trait::async_trait]
impl Handler<Pow> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Pow) {}
}

#[hannibal::main]
async fn main() -> Result<()> {
    // start new actor
    let addr = MyActor { count: 10 }.start().await?;

    let caller: Caller<Ping> = addr.caller();
    let caller2 = caller.clone();

    let sender: Sender<Pow> = addr.sender();

    assert!(sender.send(Pow).is_ok());

    let res = caller.call(Ping(10)).await?;
    println!("RESULT: {}", res == 20);

    let res = caller2.call(Ping(10)).await?;
    println!("RESULT: {}", res == 30);

    println!("caller can upgrade: {}", caller.can_upgrade());
    std::mem::drop(addr);
    println!("caller can upgrade: {}", caller.can_upgrade());
    assert!(sender.send(Pow).is_ok());

    let res = caller.call(Ping(10)).await?;
    println!("RESULT: {}", res == 20);

    Ok(())
}
