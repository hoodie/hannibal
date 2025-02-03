use std::time::Duration;

use hannibal::prelude::*;

#[derive(Debug)]
struct Child(usize);
impl Actor for Child {
    async fn started(&mut self, ctx: &mut Context<Self>) -> hannibal::DynResult<()> {
        println!("{self:?} started");
        ctx.interval_with(|| (), Duration::from_millis(300 + 300 * self.0 as u64));
        Ok(())
    }

    async fn stopped(&mut self, _: &mut Context<Self>) {
        println!("{self:?} stopped");
    }
}

impl Handler<()> for Child {
    async fn handle(&mut self, _: &mut Context<Self>, _: ()) {
        println!("{self:?} {:>width$}!", "x", width = (self.0 + 1) * 10);
    }
}

#[derive(Debug)]
struct Root;
impl Actor for Root {
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        println!("{self:?} started");
        ctx.create_child(|| Child(0));
        ctx.create_child(|| Child(1));
        ctx.create_child(|| Child(2));
        ctx.create_child(|| Child(3));
        Ok(())
    }
    async fn stopped(&mut self, _: &mut Context<Self>) {
        println!("{self:?} stopped");
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
