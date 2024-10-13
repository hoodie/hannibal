use async_signals::Signals;
use minibal::{prelude::*, register};

#[derive(Debug, Default)]
struct SignalService {
    sig_count: u8,
}

impl Actor for SignalService {}
impl Service for SignalService {}

impl StreamHandler<i32> for SignalService {
    async fn handle(&mut self, ctx: &mut Context<Self>, signal: i32) {
        self.sig_count += 1;
        println!("{}. Received signal: {:?}", self.sig_count, signal);
        if self.sig_count == 3 {
            ctx.stop().unwrap();
        }
    }

    async fn finished(&mut self, _ctx: &mut Context<Self>) {
        println!("Received {:?} signals", self.sig_count);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let signals = Signals::new(vec![libc::SIGINT]).unwrap();
    let addr = SignalService::default().spawn_on_stream(signals).unwrap();
    register(addr).await;
    let addr = SignalService::from_registry().await.unwrap();

    println!("kill me with Ctrl-C three times to stop the actor");

    addr.await.unwrap();
}
