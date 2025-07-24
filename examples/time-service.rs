use hannibal::prelude::*;

#[derive(Debug, Default, Actor, Service)]
struct TimerService {}

#[message(response = String)]
struct Retrieve;

impl Handler<Retrieve> for TimerService {
    async fn handle(&mut self, _: &mut Context<Self>, Retrieve: Retrieve) -> String {
        format!("{:?}", std::time::Instant::now())
    }
}

#[hannibal::main]
async fn main() {
    let result = TimerService::from_registry()
        .await
        .call(Retrieve)
        .await
        .unwrap();

    println!("retrieved: {result:?}");
}
