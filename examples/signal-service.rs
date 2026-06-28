use hannibal::{Signal, broadcast_signals, prelude::*};

#[derive(Default)]
struct MyActor {
    count: u8,
}

impl Actor for MyActor {
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        ctx.subscribe::<Signal>().await?;
        Ok(())
    }
}

impl Handler<Signal> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _msg: Signal) {
        let limit = 3;
        self.count += 1;
        if self.count == limit {
            ctx.stop().unwrap();
        }
    }
}

#[hannibal::main(flavor = "current_thread")]
async fn main() -> DynResult<()> {
    broadcast_signals().await?;

    println!("kill me with Ctrl-C three times to stop the actor");

    MyActor::default().spawn().await?;
    Ok(())
}
