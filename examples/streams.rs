use std::ops::Range;

use futures::stream::{self};
use hannibal::*;

#[derive(Debug)]
struct StartsStreamOnStarted(Option<futures::stream::Iter<Range<u32>>>);
impl Actor for StartsStreamOnStarted {
    async fn started(&mut self, ctx: &mut Context<Self>) -> Result<()> {
        println!("{self:?} started");
        ctx.add_stream(self.0.take().unwrap());
        Ok(())
    }
    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("{self:?} stopped");
    }
}
impl From<Range<u32>> for StartsStreamOnStarted {
    fn from(range: Range<u32>) -> Self {
        Self(Some(stream::iter(range)))
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
    // adds stream on start by itself
    StartsStreamOnStarted::from(0..10)
        .start()
        .await?
        .wait_for_stop()
        .await;

    // get stream added by lifecycle
    StopsOnFinished
        .start_with_stream(stream::iter(0..10))
        .await?
        .wait_for_stop()
        .await;

    // get stream added by lifecycle
    Passive
        .start_bound_to_stream(stream::iter(0..10))
        .await?
        .wait_for_stop()
        .await;
    Ok(())
}
