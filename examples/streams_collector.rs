use hannibal::prelude::*;

#[derive(Default, Actor)]
struct Collector<T: Send + 'static>(Vec<T>);

impl<T: Send> StreamHandler<T> for Collector<T> {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: T) {
        self.0.push(msg);
    }
}

#[hannibal::main]
async fn main() {
    let num_stream = futures::stream::iter('a'..='z');
    let mut addr = hannibal::build(Collector::default())
        .on_stream(num_stream)
        .spawn_owning();

    if let Some(Collector(collected)) = addr.join().await {
        println!("collected values: {collected:?}");
    }
}
