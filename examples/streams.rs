use hannibal::prelude::*;

#[derive(Actor, Debug, Default)]
struct FizzBuzzer(&'static str);

impl StreamHandler<i32> for FizzBuzzer {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
        match (msg % 3 == 0, msg % 5 == 0) {
            (true, true) => {
                self.0 = "fizzbuzz";
                println!("{msg} -> {inner}", inner = self.0, msg = msg);
            }
            (true, false) => {
                self.0 = "fizz";
                println!("{msg} -> {inner}", inner = self.0, msg = msg);
            }
            (false, true) => {
                self.0 = "buzz";
                println!("{msg} -> {inner}", inner = self.0, msg = msg);
            }
            _ => {}
        }
    }
}

#[hannibal::main]
async fn main() {
    let mut addr = hannibal::build(FizzBuzzer::default())
        .on_stream(futures::stream::iter(1..30))
        .spawn_owning();

    {
        let new = dbg!(addr.await.unwrap());

        dbg!(
            hannibal::build(new)
                .on_stream(futures::stream::iter(1..30))
                .spawn_owning()
                .await
        );
    }
}
