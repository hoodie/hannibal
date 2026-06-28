use hannibal::prelude::*;

// does not implement Handler<Stop> explicitly
struct Worker(&'static str);

#[message]
struct DoWork(&'static str);

impl Actor for Worker {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult<()> {
        println!("[{}] started", self.0);
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("[{}] stopped", self.0);
    }
}

impl Handler<DoWork> for Worker {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: DoWork) {
        println!("[{}] working on: {}", self.0, msg.0);
    }
}

#[hannibal::main]
async fn main() {
    env_logger::init();

    let addr = Worker("alpha").spawn();

    addr.send(DoWork("task-1")).await.unwrap();
    addr.send(DoWork("task-2")).await.unwrap();

    addr.stop().unwrap();
    addr.send(hannibal::Stop).await.unwrap();


    // Wait for the actor to finish.
    addr.await.unwrap();

    println!("done");
}
