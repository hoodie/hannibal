#![cfg(feature = "tokio")]
use std::time::Duration;

use minibal::prelude::*;

struct MyActor(u8);

struct Stop;
impl Message for Stop {
    type Response = ();
}

impl Actor for MyActor {
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        println!("[Actor] started");
        ctx.interval((), Duration::from_secs(1));
        ctx.delayed_send(|| Stop, Duration::from_secs(5));
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("[Actor] stopped");
    }
}

impl Handler<()> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: ()) {
        self.0 += 1;
        println!("[Actor] received interval message {}", self.0);
    }
}

impl Handler<Stop> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _msg: Stop) {
        println!("[Actor] received stop message");
        ctx.stop().unwrap();
    }
}

#[tokio::main]
async fn main() {
    MyActor(0).spawn().await.unwrap();
}
