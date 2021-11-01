use hannibal::*;

/// Define `Ping` message
#[message(result = "usize")]
struct Ping(usize);

/// Actor
struct MyActor;

/// Declare actor and its context
#[async_trait::async_trait]
impl Actor for MyActor {
    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("stopped");
    }
}

#[hannibal::main]
async fn main() -> Result<()> {
    // start new actor
    let mut addr = MyActor.start().await?;
    let addr2 = addr.clone();
    let addr3 = addr.clone();
    addr.stop(None).unwrap();
    println!("addr3 stopped {}", addr3.stopped());

    println!("waiting to stop");
    addr.wait_for_stop().await;
    println!("enough waiting");
    addr2.wait_for_stop().await;
    println!("waited for clone");

    println!("addr3 stopped {}", addr3.stopped());

    Ok(())
}
