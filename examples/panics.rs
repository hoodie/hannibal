#![allow(clippy::dbg_macro)]

use hannibal::prelude::*;

#[derive(Actor)]
struct Hitchhiker;

#[message(response = String)]
struct Echo(String);

impl Handler<Echo> for Hitchhiker {
    async fn handle(&mut self, _ctx: &mut Context<Self>, Echo(msg): Echo) -> String {
        msg
    }
}

#[message]
struct Boo;

impl Handler<Boo> for Hitchhiker {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _: Boo) {
        panic!("Oh no");
    }
}

#[hannibal::main]
async fn main() -> DynResult<()> {
    let hitchhiker = Hitchhiker.spawn();

    println!(
        "Hitchhiker says {:?}",
        hitchhiker.call(Echo("Hello".into())).await?
    );

    _ = hitchhiker.send(Boo).await;

    assert!(hitchhiker.stopped());
    assert!(hitchhiker.ping().await.is_err());

    Ok(())
}
