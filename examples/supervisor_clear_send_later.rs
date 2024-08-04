use hannibal::{message, Actor, Context, Handler};
use std::time::Duration;

#[derive(Debug, Default)]
pub struct PingLater;

impl Actor for PingLater {
    async fn started(&mut self, ctx: &mut Context<Self>) -> hannibal::Result<()> {
        ctx.send_later(Ping("after halt"), Duration::from_millis(1_500));

        Ok(())
    }
    /// Called after an actor is stopped.
    async fn stopped(&mut self, _: &mut Context<Self>) {
        println!("PingLater:: stopped()");
    }
}

#[message]
#[derive(Debug)]
struct Ping(&'static str);

impl Handler<Ping> for PingLater {
    async fn handle(&mut self, _ctx: &mut Context<Self>, Ping(msg): Ping) {
        println!("PingLater:: handle {:?}", msg);
    }
}
#[message]
struct Halt;

impl Handler<Halt> for PingLater {
    async fn handle(&mut self, ctx: &mut Context<Self>, _msg: Halt) {
        println!("PingLater:: received Halt");
        ctx.stop(None);
        println!("PingLater:: stopped");
    }
}

#[hannibal::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service_supervisor = hannibal::Supervisor::start(PingLater::default).await?;
    let service_addr = service_supervisor.clone();

    let supervisor_task = hannibal::spawn(async {
        service_supervisor.wait_for_stop().await;
    });

    let send_ping = async {
        println!("  main  :: sending Ping");
        service_addr.send(Ping("before halt")).unwrap();
    };

    let send_halt = async {
        hannibal::sleep(Duration::from_millis(1_000)).await;
        println!("  main  :: sending Halt");
        service_addr.send(Halt).unwrap();
    };

    let _ = futures::join!(supervisor_task, send_halt, send_ping);

    Ok(())
}
