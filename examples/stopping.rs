use hannibal::*;
use std::time::Duration;

#[message]
struct Die;

struct MyActor(&'static str);

impl Actor for MyActor {
    async fn started(&mut self, ctx: &mut Context<Self>) -> Result<()> {
        // Send the Die message 3 seconds later
        ctx.send_later(Die, Duration::from_secs(3));
        Ok(())
    }

    async fn stopping(&mut self, _ctx: &mut Context<Self>, reason: StopReason) {
        if let Some(reason) = reason {
            println!("✋ stopping {} with reason: {}", self.0, reason);
        } else {
            println!("✋ stopping {} without reason", self.0);
        }
    }
    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("🛑 actually {} stopped", self.0);
    }
}

impl Handler<Die> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _msg: Die) {
        ctx.stop(reason("timeout"));
    }
}

fn reason(msg: &str) -> StopReason {
    Some(Box::<dyn std::error::Error + Send + Sync>::from(msg))
}

#[hannibal::main]
async fn main() -> Result<()> {
    let mut a1 = MyActor("1").start().await?;
    let mut a2 = MyActor("2").start().await?;
    // Exit the program after 3 seconds
    let a3 = MyActor("3").start().await?;

    a1.stop(reason("halt and catch fire")).unwrap();
    a2.stop(None).unwrap();

    futures::join!(a1.wait_for_stop(), a2.wait_for_stop(), a3.wait_for_stop(),);

    Ok(())
}
