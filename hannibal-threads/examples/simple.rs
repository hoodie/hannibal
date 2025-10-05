use std::sync::{Arc, RwLock};

use hannibal_threads::{ActorResult, EventLoop, actor::Actor, handler::Handler};

struct MyActor(&'static str);

impl Actor for MyActor {
    fn started(&mut self) -> ActorResult<()> {
        println!("[Actor {}] started", self.0);
        Ok(())
    }

    fn stopped(&mut self) -> ActorResult<()> {
        println!("[Actor {}] stopped", self.0);
        Ok(())
    }
}

impl Handler<i32> for MyActor {
    fn handle(&mut self, msg: i32) {
        println!("[Actor {}] received an number {}", self.0, msg);
    }
}

impl Handler<String> for MyActor {
    fn handle(&mut self, msg: String) {
        println!("[Actor {}] received a string {}", self.0, msg);
    }
}

fn halt() {
    eprintln!("Press Enter to exit");
    std::io::stdin().read_line(&mut String::new()).unwrap();
}

fn internal_api() {
    let actor = Arc::new(RwLock::new(MyActor("actor 0")));
    let mut life_cycle = EventLoop::default();
    life_cycle.spawn(actor.clone()).unwrap();

    let ctx = life_cycle.ctx;

    ctx.send(String::from("hello world"), actor.clone())
        .unwrap();
    ctx.send(1337, actor.clone()).unwrap();
    ctx.send(4711, actor.clone()).unwrap();
    ctx.stop().unwrap();
}

fn main() {
    internal_api();

    let addr1 = EventLoop::default().start(MyActor("actor 1")).unwrap();
    let addr2 = EventLoop::default().start(MyActor("actor 2")).unwrap();
    let sender = addr1.sender::<i32>();
    let sender2 = addr2.sender::<i32>();

    addr1.send(42).unwrap();
    sender.send(43).unwrap();

    let weak_sender = sender.downgrade();
    weak_sender.try_send(44).unwrap();

    drop(addr1);
    drop(sender);
    eprintln!("{}:{} {:?}", file!(), line!(), weak_sender.try_send(45));
    halt();

    sender2.send(46).unwrap();
    addr2.stop().unwrap();
    sender2.send(47).unwrap(); // lost TODO: make

    halt();
}
