use hannibal::*;
use std::time::Duration;

#[message]
struct Die;

struct MyActor;

impl Actor for MyActor {
    async fn started(&mut self, ctx: &mut Context<Self>) -> Result<()> {
        // Send the Die message 3 seconds later
        ctx.send_later(Die, Duration::from_secs(3));
        Ok(())
    }
}

impl Handler<Die> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _msg: Die) {
        // Stop the actor without error
        ctx.stop(None);
    }
}

#[hannibal::main]
async fn main() -> Result<()> {
    // Exit the program after 3 seconds
    let addr = MyActor.start().await?;
    addr.wait_for_stop().await;
    Ok(())
}
