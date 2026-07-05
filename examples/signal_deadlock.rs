/// Reproducer for deadlock when multiple actors subscribe to `hannibal::Signal`,
/// where one actor is a registered service.
///
/// Expected: both actors stop cleanly on Ctrl-C.
/// Actual:   process hangs.
use hannibal::prelude::*;

struct AlphaActor;

impl Actor for AlphaActor {
    async fn started(&mut self, ctx: &mut Context<Self>) -> hannibal::DynResult<()> {
        ctx.subscribe::<hannibal::Signal>().await?;
        Ok(())
    }
}

impl Handler<hannibal::Signal> for AlphaActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _signal: hannibal::Signal) {
        println!("AlphaActor received signal, stopping");
        ctx.stop().unwrap();
    }
}

// BetaActor is a service
#[derive(Default)]
struct BetaActor;

impl Actor for BetaActor {
    async fn started(&mut self, ctx: &mut Context<Self>) -> hannibal::DynResult<()> {
        ctx.subscribe::<hannibal::Signal>().await?;
        Ok(())
    }
}

impl Service for BetaActor {}

impl Handler<hannibal::Signal> for BetaActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _signal: hannibal::Signal) {
        println!("BetaActor received signal, stopping");
        ctx.stop().unwrap();
    }
}

#[tokio::main]
async fn main() -> DynResult<()> {
    hannibal::broadcast_signals().await?;
    BetaActor::setup().await.unwrap();
    let alpha = AlphaActor.spawn();

    println!("Both actors running, press Ctrl-C to stop");

    alpha.await?;

    println!("Clean shutdown");
    Ok(())
}
