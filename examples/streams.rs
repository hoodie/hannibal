use hannibal::*;

struct MyActor;

impl StreamHandler<u32> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: u32) {
        println!("Received: {}", msg);
    }

    async fn started(&mut self, _ctx: &mut Context<Self>) {
        println!("stream started");
    }

    async fn finished(&mut self, ctx: &mut Context<Self>) {
        println!("stream finished");
        ctx.stop(None);
    }
}

impl Actor for MyActor {
    async fn started(&mut self, ctx: &mut Context<Self>) -> Result<()> {
        ctx.add_stream(futures::stream::iter(0..100));
        Ok(())
    }
    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("actor stopped");
    }
}

#[hannibal::main]
async fn main() -> Result<()> {
    let addr = MyActor.start().await?;
    addr.wait_for_stop().await;
    Ok(())
}
