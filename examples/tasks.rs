use std::time::Duration;

use hannibal::prelude::*;

struct MyActor(&'static str);

#[message]
struct Work;

impl Actor for MyActor {
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        println!("[Actor {}] started", self.0);
        let mut me = ctx.weak_address().ok_or("Sorry")?;
        ctx.delayed_exec(
            async move {
                me.try_halt().await.unwrap();
                println!("shut down root");
            },
            Duration::from_secs(5),
        );
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("[Actor {}] stopped", self.0);
    }
}

impl Handler<Work> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _msg: Work) {
        log::info!("been told to work");

        ctx.spawn_task(async { slow_task().await });
    }
}

async fn slow_task() {
    log::info!("Slow Task Started");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    log::info!("Slow Task Finished");
}

#[hannibal::main]
async fn main() {
    env_logger::init();
    // let (mut addr, _handle) = MyActor("Caesar").spawn_with::<SmolSpawner>();
    let mut addr = MyActor("Caesar").spawn();

    // send a message without a response
    addr.send(Work).await.unwrap();

    println!("{:#?}", addr.stop());
}
