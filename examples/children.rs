use std::time::Duration;

use minibal::prelude::*;

struct Child(usize);
impl Actor for Child {
    async fn started(&mut self, ctx: &mut Context<Self>) -> minibal::DynResult<()> {
        ctx.start_interval_of(|| {}, Duration::from_secs(1));
        Ok(())
    }

    async fn stopped(&mut self, _: &mut Context<Self>) {
        println!("Child stopped {}", self.0);
    }
}

impl Handler<()> for Child {
    async fn handle(&mut self, _: &mut Context<Self>, _: ()) {
        println!("Child {} received message", self.0);
    }
}

struct Root;
impl Actor for Root {
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        ctx.create_child(|| Child(0));
        ctx.create_child(|| Child(1));
        ctx.create_child(|| Child(2));
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut root_addr = Root.spawn();

    tokio::time::sleep(Duration::from_secs(5)).await;

    root_addr.stop()?;

    root_addr.await?;
    Ok(())
}
