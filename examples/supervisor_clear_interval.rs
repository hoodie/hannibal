use hannibal::{message, Actor, Context, Handler};
use std::time::Duration;

#[derive(Debug)]
pub struct PingTimer;

impl Actor for PingTimer {
    async fn started(&mut self, ctx: &mut Context<Self>) -> hannibal::Result<()> {
        println!("PingTimer :: started()");
        ctx.send_interval(Ping, Duration::from_millis(300));
        Ok(())
    }

    /// Called after an actor is stopped.
    async fn stopped(&mut self, _: &mut Context<Self>) {
        println!("PingTimer :: stopped()");
    }
}

#[message]
#[derive(Clone)]
struct Ping;

impl Handler<Ping> for PingTimer {
    async fn handle(&mut self, _: &mut Context<Self>, _msg: Ping) {
        println!("PingTimer :: Ping");
    }
}
#[message]
struct Restart;

impl Handler<Restart> for PingTimer {
    async fn handle(&mut self, ctx: &mut Context<Self>, _msg: Restart) {
        println!("PingTimer :: received restart");
        ctx.stop(None);
    }
}

#[message]
struct Shutdown;

impl Handler<Shutdown> for PingTimer {
    async fn handle(&mut self, ctx: &mut Context<Self>, _msg: Shutdown) {
        println!("PingTimer :: received Shutdown");
        ctx.stop_supervisor(None);
    }
}

#[message]
struct Panic;

impl Handler<Panic> for PingTimer {
    async fn handle(&mut self, _: &mut Context<Self>, _msg: Panic) {
        println!("PingTimer :: received Panic");
        panic!("intentional panic: this should not occur");
    }
}

#[hannibal::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service_supervisor = hannibal::Supervisor::start(|| PingTimer).await?;
    let service_addr = service_supervisor.clone();
    let service_addr2 = service_supervisor.clone();

    let supervisor_task = hannibal::spawn(async {
        service_supervisor.wait_for_stop().await;
    });

    let stop_actor = async {
        hannibal::sleep(Duration::from_millis(2_000)).await;
        println!("   main   :: sending Restart");
        service_addr.send(Restart).unwrap();
    };

    let stop_supervisor = async move {
        hannibal::sleep(Duration::from_millis(3_000)).await;
        println!("   main   :: sending Shutdown");
        service_addr2.send(Shutdown).unwrap();
    };

    let send_panic = async {
        hannibal::sleep(Duration::from_millis(5_000)).await;
        println!("   main   :: sending Panic after stop");
        if let Err(error) = service_addr.send(Panic) {
            println!("    ok    :: cannot send after halting, this is very much expected");
            println!("             Failing with \"{}\"", error);
        }
    };

    futures::join!(
        supervisor_task,
        stop_actor,
        stop_supervisor,
        send_panic, // there is no panic recovery
    );

    Ok(())
}
