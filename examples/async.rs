use std::sync::Arc;

use minibal::{Actor, EventLoop, Handler};

struct MyActor(&'static str);

impl Actor for MyActor {
    fn started(&self) {
        println!("[Actor {}] started", self.0);
    }
    fn stopped(&self) {
        println!("[Actor {}] stopped", self.0);
    }
}

impl Handler<i32> for MyActor {
    fn handle(&self, msg: i32) {
        println!("[Actor {}] received an number {}", self.0, msg);
    }
}

impl Handler<String> for MyActor {
    fn handle(&self, msg: String) {
        println!("[Actor {}] received a string {}", self.0, msg);
    }
}

fn halt() {
    eprintln!("Press Enter to exit");
    std::io::stdin().read_line(&mut String::new()).unwrap();
}

async fn internal_api() {
    let actor = Arc::new(MyActor("actor 0"));
    let mut life_cycle = EventLoop::default();
    life_cycle.spawn(actor.clone()).unwrap();

    let ctx = life_cycle.ctx;

    ctx.send(String::from("hello world"), actor.clone()).await;
    ctx.send(1337, actor.clone()).await;
    ctx.send(4711, actor.clone()).await;
    ctx.stop().await;
}

#[async_std::main]
async fn main() {
    color_backtrace::install();
    internal_api().await;

    let addr1 = EventLoop::default().start(MyActor("actor 1"));
    let addr2 = EventLoop::default().start(MyActor("actor 2"));
    let sender = addr1.sender::<i32>();
    let sender2 = addr2.sender::<i32>();

    // addr1.send(42)
    sender.send(43).await;

    let weak_sender = sender.downgrade();
    weak_sender.try_send(44).await;

    drop(addr1);
    drop(sender);
    dbg!(weak_sender.try_send(45).await);
    halt();

    sender2.send(46).await;
    addr2.stop();
    sender2.send(47).await; // lost TODO: make

    halt();
}
