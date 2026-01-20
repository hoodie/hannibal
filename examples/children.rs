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

#[derive(Clone, Message)]
struct Hello;
impl Handler<Hello> for Child {
    async fn handle(&mut self, _: &mut Context<Self>, _: Hello) {
        println!("Hello I'm child {}", self.0);
    }
}

impl Handler<Hello> for Root {
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: Hello) {
        println!("Greeting My Children");
        ctx.send_to_children(msg);
    }
}

impl Handler<()> for Child {
    async fn handle(&mut self, _: &mut Context<Self>, _: ()) {
        println!("{self:?} {:>width$}!", "x", width = (self.0 + 1) * 10);
    }
}

// #[derive(Clone, Message)]
// struct Gc;
// impl Handler<Gc> for Root {
//     async fn handle(&mut self, ctx: &mut Context<Self>, _: Gc) {
//         println!("Garbage Collecting");
//         ctx.gc();
//     }
// }

#[derive(Debug)]
struct Root;
impl Actor for Root {
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        println!("{self:?} started");
        ctx.add_child(Child(0).spawn());
        ctx.add_child(Child(1).spawn());
        ctx.add_child(Child(2).spawn());

        ctx.register_child::<Hello>(Child(3).spawn());
        ctx.register_child::<Hello>(Child(4).spawn());
        ctx.register_child::<Hello>(Child(5).spawn());

        ctx.interval_with(|| Hello, Duration::from_millis(500));
        ctx.interval(hannibal::messages::Gc, Duration::from_millis(500));

        let mut me = ctx.weak_address();
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

#[hannibal::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    Root.spawn().await?;
    Ok(())
}
