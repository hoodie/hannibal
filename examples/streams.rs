use hannibal::prelude::*;

#[derive(Actor, Debug, Default)]
struct FizzBuzzer(&'static str, usize);

impl StreamHandler<i32> for FizzBuzzer {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
        self.1 += 1;
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // create a new `FizzBuzzer` and start it on a stream
    let mut addr = hannibal::build(FizzBuzzer::default())
        .on_stream(futures::stream::iter(1..30))
        .spawn_owning();

    // get the actor back after the stream is finished
    let fizzbuzz = addr.join().await.unwrap();
    println!("fizz buzz done {fizzbuzz:?}");

    // spawn the actor with the same address
    let fizzbuzz = hannibal::build(fizzbuzz)
        .on_stream(futures::stream::iter(1..30))
        .spawn_owning()
        .await?;
    println!("fizz buzz done {fizzbuzz:?}");

    Ok(())
}
