use async_signals::Signals;
use minibal::prelude::*;

#[derive(Debug, Default)]
struct SignalService {
    sig_count: u8,
}

impl Actor for SignalService {}
impl Service for SignalService {
    async fn setup() -> DynResult<()> {
        let signals = Signals::new([libc::SIGINT])?;

        SignalService::default()
            .spawn_on_stream(signals)?
            .register()
            .await?;
        Ok(())
    }
}

impl StreamHandler<i32> for SignalService {
    async fn handle(&mut self, ctx: &mut Context<Self>, signal: i32) {
        let limit = 3;
        self.sig_count += 1;
        println!("Received signal {signal:?}: {}/{limit}", self.sig_count);
        if self.sig_count == limit {
            ctx.stop().unwrap();
        }
    }

    async fn finished(&mut self, _ctx: &mut Context<Self>) {
        println!("Received {:?} signals", self.sig_count);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    SignalService::setup().await.unwrap();

    let addr = SignalService::from_registry().await;

    println!("kill me with Ctrl-C three times to stop the actor");

    addr.await.unwrap();
}
