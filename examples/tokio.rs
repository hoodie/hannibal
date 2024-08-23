use std::pin::Pin;

use minibal::non_blocking::{Actor, EventLoop as EventLoop, Handler as Handler};
use minibal::ActorResult;

struct MyActor(&'static str);

impl Actor for MyActor {
    async fn started(&mut self) -> ActorResult<()> {
        println!("[Actor {}] started", self.0);
        Ok(())
    }

    async fn stopped(&mut self) -> ActorResult<()> {
        println!("[Actor {}] stopped", self.0);
        Ok(())
    }
}

impl Handler<i32> for MyActor {
    fn handle(&mut self, msg: i32) -> impl std::future::Future<Output = ActorResult<()>> + Send {
        println!("[Actor {}] received an number {}", self.0, msg);
    }
}

impl Handler<String> for MyActor {
    fn handle(&mut self, msg: String) -> impl std::future::Future<Output = ActorResult<()>> + Send {
        println!("[Actor {}] received a string {}", self.0, msg);
    }
}

#[tokio::main]
async fn main() {
    let addr1 = EventLoop::default().start(MyActor("actor 1")).unwrap();
    let addr2 = EventLoop::default().start(MyActor("actor 2")).unwrap();
    let sender = addr1.sender::<i32>();
    let sender2 = addr2.sender::<i32>();

    addr1.send(42).unwrap();
    sender.send(43).unwrap();

    let weak_sender = sender.downgrade();
    weak_sender.try_send(44).unwrap();
}
