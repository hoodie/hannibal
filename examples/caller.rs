use hannibal::*;

/// Define `Ping` message
#[message(result = usize)]
struct Ping(usize);

#[message]
struct Pow;

/// Actor
struct CountActor {
    count: usize,
}

/// Declare actor and its context
impl Actor for CountActor {}

/// Handler for `Ping` message
impl Handler<Ping> for CountActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Ping) -> usize {
        self.count += msg.0;
        self.count
    }
}

/// Handler for `Ping` message
impl Handler<Pow> for CountActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Pow) {}
}

#[hannibal::main]
async fn main() -> Result<()> {
    color_backtrace::install();
    // start new actor
    let addr = CountActor { count: 10 }.start().await?;

    let caller: Caller<Ping> = addr.caller();
    let caller2 = caller.clone();

    let sender: Sender<Pow> = addr.sender();

    assert!(sender.send(Pow).is_ok());

    let res = caller.call(Ping(10)).await?;
    println!("RESULT: {}", res == 20);
    assert_eq!(res, 20);

    let res = caller2.call(Ping(10)).await?;
    println!("RESULT: {}", res == 30);
    assert_eq!(res, 30);

    println!("caller can upgrade: {}", caller.can_upgrade());
    assert!(caller.can_upgrade());

    std::mem::drop(addr);

    assert!(!caller.can_upgrade());
    assert!(sender.send(Pow).is_err());

    let res = caller.call(Ping(10)).await?;
    println!("RESULT: {}", res == 20);

    Ok(())
}
