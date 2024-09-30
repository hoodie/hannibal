use std::future::Future;

use minibal::{Actor, ActorResult, Addr, Context, Handler, Message};

#[derive(Debug, Default)]
struct MyActor(Option<&'static str>);

struct Stop;
impl Message for Stop {
    type Result = ();
}

struct Store(&'static str);
impl Message for Store {
    type Result = ();
}

struct Add(i32, i32);
impl Message for Add {
    type Result = i32;
}

impl Actor for MyActor {}

impl Handler<Stop> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _: Stop) {
        if let Err(e) = ctx.stop() {
            eprintln!("{}", e);
        }
    }
}

impl Handler<Store> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Store) {
        self.0.replace(msg.0);
    }
}

impl Handler<Add> for MyActor {
    async fn handle(&mut self, _: &mut Context<Self>, msg: Add) -> i32 {
        msg.0 + msg.1
    }
}

fn start<A: Actor>(actor: A) -> (impl Future<Output = ActorResult<A>>, Addr<A>) {
    let (event_loop, addr) = minibal::Environment::unbounded().launch(actor);
    (event_loop, addr)
}

#[tokio::test]
async fn addr_call() {
    let (event_loop, addr) = start(MyActor::default());
    tokio::spawn(event_loop);

    let addition = addr.call(Add(1, 2)).await.unwrap();
    assert_eq!(addition, 3);
}

#[tokio::test]
async fn addr_send() {
    let (event_loop, mut addr) = start(MyActor::default());
    let task = tokio::spawn(event_loop);
    addr.send(Store("password")).unwrap();
    addr.stop().unwrap();
    let actor = task.await.unwrap().unwrap();
    assert_eq!(actor.0, Some("password"))
}

#[tokio::test]
async fn addr_send_err() {
    let (event_loop, mut addr) = start(MyActor::default());
    tokio::spawn(event_loop);
    let addr2 = addr.clone();
    addr.stop().unwrap();
    addr.await.unwrap();
    assert!(addr2.send(Store("password")).is_err());
}

#[tokio::test]
async fn addr_stop() {
    let (event_loop, mut addr) = start(MyActor::default());
    tokio::spawn(event_loop);

    let addr2 = addr.clone();
    addr.stop().unwrap();

    addr2.await.unwrap();
    addr.await.unwrap();
}

#[tokio::test]
async fn ctx_stop() {
    let (event_loop, addr) = start(MyActor::default());
    tokio::spawn(event_loop);

    let addr2 = addr.clone();
    addr.send(Stop).unwrap();

    addr2.await.unwrap();
    addr.await.unwrap();
}

#[tokio::test]
async fn addr_stopped_after_stop() {
    let (event_loop, addr) = start(MyActor::default());
    tokio::spawn(event_loop);

    let addr2 = addr.clone();
    assert!(!addr2.stopped(), "addr2 should not be stopped");

    addr.send(Stop).unwrap();

    addr.await.unwrap();
    assert!(addr2.stopped(), "addr2 should be stopped");
}
