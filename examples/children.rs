use minibal::prelude::*;

struct Child(usize);
impl Actor for Child {
    async fn stopped(&mut self, _: &mut Context<Self>) {
        println!("Child stopped {}", self.0);
    }
}
impl Handler<()> for Child {
    async fn handle(&mut self, _: &mut Context<Self>, _: ()) {}
}

struct Root;
impl Actor for Root {
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        ctx.create_child(|| Child(0))?;
        ctx.create_child(|| Child(1))?;
        ctx.create_child(|| Child(2))?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut root_addr = Root.spawn()?;

    root_addr.stop()?;

    root_addr.await?;
    Ok(())
}
