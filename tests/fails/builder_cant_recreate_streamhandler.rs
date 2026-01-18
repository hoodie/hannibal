use futures::stream;
use hannibal::{RestartableActor, prelude::*};

#[derive(Default)]
struct MyActor(&'static str);
impl Actor for MyActor {}

impl StreamHandler<i32> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
        println!("[Actor {}] Received: {}", self.0, msg);
    }
}

impl RestartableActor for MyActor {}

#[tokio::main]
async fn main() {
    // This should fail to compile: on_stream is not available after recreate_from_default
    let addr = hannibal::setup_actor(MyActor("Caesar"))
        .recreate_from_default()
        .on_stream(stream::iter(vec![17, 19]))
        .spawn();
    addr.await.unwrap();
}
