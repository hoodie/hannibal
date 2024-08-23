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

    let caller: WeakCaller<Ping> = addr.weak_caller();
    let caller2 = caller.clone();

    let sender: WeakSender<Pow> = addr.weak_sender();

    assert!(sender.try_send(Pow).is_ok());

    let res = caller.try_call(Ping(10)).await?;
    println!("RESULT: {}", res == 20);
    assert_eq!(res, 20);

    let res = caller2.try_call(Ping(10)).await?;
    println!("RESULT: {}", res == 30);
    assert_eq!(res, 30);

    println!("caller can upgrade: {}", caller.upgrade().is_some());
    assert!(caller.upgrade().is_some());

    std::mem::drop(addr);

    assert_eq!(caller.upgrade().is_some(), false);
    assert!(sender.try_send(Pow).is_err());

    let res = caller.try_call(Ping(10)).await?;
    println!("RESULT: {}", res == 20);

    Ok(())
}
