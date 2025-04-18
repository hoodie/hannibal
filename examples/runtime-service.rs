use hannibal::prelude::*;

#[derive(Debug, Default, Actor, Service)]
struct TimerService {}

#[message(response = Option<String>)]
struct Retrieve;

impl Handler<Retrieve> for TimerService {
    async fn handle(&mut self, _: &mut Context<Self>, Retrieve: Retrieve) -> Option<String> {
        std::thread::current().name().map(ToOwned::to_owned)
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let result = TimerService::from_registry()
        .await
        .call(Retrieve)
        .await
        .unwrap();

    println!("retrieved: {:?}", result);
}
