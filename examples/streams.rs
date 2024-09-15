use hannibal::*;

#[derive(Debug)]
struct StartsStreamOnStarted;
impl Actor for StartsStreamOnStarted {
    async fn started(&mut self, ctx: &mut Context<Self>) -> Result<()> {
        println!("{self:?} started");
        ctx.add_stream(futures::stream::iter(0..10));
        Ok(())
    }
    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("{self:?} stopped");
    }
}

impl StreamHandler<u32> for StartsStreamOnStarted {
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

#[derive(Debug)]
struct StopsOnFinished;
impl Actor for StopsOnFinished {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> Result<()> {
        println!("{self:?} started");
        Ok(())
    }
    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("{self:?} stopped");
    }
}

impl StreamHandler<u32> for StopsOnFinished {
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

#[derive(Debug)]
struct Passive;
impl Actor for Passive {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> Result<()> {
        println!("{self:?} started");
        Ok(())
    }
    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("{self:?} stopped");
    }
}

impl StreamHandler<u32> for Passive {
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

#[hannibal::main]
async fn main() -> Result<()> {
    // adds stream on start
    let addr = StartsStreamOnStarted.start().await?;
    addr.wait_for_stop().await;

    // get stream added by lifecycle
    let addr = StopsOnFinished
        .start_with_stream(futures::stream::iter(0..10))
        .await?;
    addr.wait_for_stop().await;

    // get stream added by lifecycle
    let addr = Passive
        .start_with_stream_efficient(futures::stream::iter(0..10))
        .await?;

    addr.wait_for_stop().await;
    Ok(())
}
