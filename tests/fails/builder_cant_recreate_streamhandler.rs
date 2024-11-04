use futures::stream;
use minibal::{prelude::*, RestartableActor};

#[derive(Default)]
struct MyActor(&'static str);
impl Actor for MyActor {}

impl StreamHandler<i32> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
        println!("[Actor {}] Received: {}", self.0, msg);
    }
}
// TODO: can we encode the restart strategy in an associated type or as a trait function?
impl RestartableActor for MyActor {}

#[tokio::main]
async fn main() {
    let addr = minibal::build(MyActor("Caesar"))
        .unbounded()
        .recreate_from_default()
        .with_stream(stream::iter(vec![17, 19])) // this shouldn't work
        .spawn();
    addr.await
}
