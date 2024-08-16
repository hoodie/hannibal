use std::sync::Arc;

use minibal::{Actor, Context, Handler, LifeCycle};

struct MyActor(&'static str);

impl Actor for MyActor {
    fn started(&self) {
        println!("[Actor {}] started", self.0);
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

fn main() {
    let mut ctx = Context::new();

    let actor = Arc::new(MyActor("actor 1"));
    ctx.spawn(actor.clone());

    let addr = LifeCycle::new().start(MyActor("actor 2").into());
    let sender = addr.sender::<i32>();

    ctx.send(String::from("hello world"), actor.clone());
    ctx.send(1337, actor.clone());
    ctx.send(4711, actor.clone());

    addr.send(42);
    sender.send(43);
    let weak_sender = sender.downgrade();
    weak_sender.try_send(44);

    drop(addr);
    drop(sender);

    dbg!(weak_sender.try_send(45));

    eprintln!("Press Enter to exit");
    std::io::stdin().read_line(&mut String::new()).unwrap();
}
