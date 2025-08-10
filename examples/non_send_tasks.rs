use std::{rc::Rc, sync::Arc};

use hannibal::prelude::*;

async fn send_task() -> Arc<String> {
    println!("send task");
    let foo = Arc::new(String::from("hello"));
    std::future::ready("work").await;
    println!("send task done");
    foo
}

async fn non_send_task() -> Rc<String> {
    println!("non send task");
    let foo = Rc::new(String::from("hello"));
    std::future::ready("work").await;
    println!("non send task done");
    foo
}

struct MyActor;

#[message]
struct StartSend;

#[message]
struct StartNonSend;

#[message]
struct LocalMessage(String);

impl Actor for MyActor {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult<()> {
        println!("[Actor] started");
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("[Actor] stopped");
    }
}

impl Handler<LocalMessage> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, LocalMessage(msg): LocalMessage) {
        println!(r#"executed a !Send task \0/ {msg}"#);
        ctx.stop().unwrap()
    }
}

impl Handler<StartSend> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _msg: StartSend) {
        let back = ctx.weak_sender();
        println!("starting local sub task");
        ctx.spawn_task(async move {
            println!(" ... send sub task ...");
            let hello = Arc::into_inner(send_task().await).unwrap();
            println!(" ... send sub task finished ...");
            back.try_send(LocalMessage(hello)).await.unwrap();
        });
    }
}

impl Handler<StartNonSend> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _msg: StartNonSend) {
        let back = ctx.weak_sender();
        println!("starting local sub task");

        ctx.spawn_task_local(async move {
            println!(" ... local sub task ...");
            let hello = Rc::into_inner(non_send_task().await).unwrap();
            println!(" ... local sub task finished ...");
            back.try_send(LocalMessage(hello)).await.unwrap();
        });
        println!("         local sub task spawned");
    }
}

#[hannibal::main]
async fn main() {
    env_logger::init();
    let addr = MyActor.spawn();

    // addr.send(StartSend).await.unwrap(); // this terminates
    addr.send(StartNonSend).await.unwrap(); // this doesn't (in tokio)
    addr.await.unwrap();
}
