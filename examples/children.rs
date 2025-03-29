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

        let mut me = ctx.weak_address().ok_or("Sorry")?;
        ctx.delayed_exec(
            async move {
                me.try_halt().await.unwrap();
                println!("shut down root");
            },
            Duration::from_secs(5),
        );
        Ok(())
    }
    async fn stopped(&mut self, _: &mut Context<Self>) {
        println!("{self:?} stopped");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Root.spawn().await?;
    Ok(())
}
