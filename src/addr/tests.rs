#![allow(clippy::unwrap_used)]
use hannibal_derive::message;

use crate::{Context, DynResult, event_loop::EventLoop};

use super::*;
use std::future::Future;

#[derive(Debug, Default)]
pub struct MyActor(pub Option<&'static str>);

#[message]
pub struct Stop;

#[message]
pub struct Store(pub &'static str);

#[message(response=i32)]
pub struct Add(pub i32, pub i32);

impl Actor for MyActor {}

impl Handler<Stop> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _: Stop) {
        if let Err(error) = ctx.stop() {
            eprintln!("{error}");
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

pub fn start<A: Actor>(actor: A) -> (impl Future<Output = DynResult<A>>, Addr<A>) {
    let (event_loop, addr) = EventLoop::unbounded().create(actor);
    (event_loop, addr)
}

#[test_log::test(tokio::test)]
async fn addr_call() {
    let (event_loop, addr) = start(MyActor::default());
    tokio::spawn(event_loop);

    let addition = addr.call(Add(1, 2)).await.unwrap();
    assert_eq!(addition, 3);
}

#[test_log::test(tokio::test)]
async fn addr_send() {
    let (event_loop, mut addr) = start(MyActor::default());
    let task = tokio::spawn(event_loop);
    addr.send(Store("password")).await.unwrap();
    addr.stop().unwrap();
    let actor = task.await.unwrap().unwrap();
    assert_eq!(actor.0, Some("password"))
}

#[test_log::test(tokio::test)]
async fn addr_send_err() {
    let (event_loop, mut addr) = start(MyActor::default());
    tokio::spawn(event_loop);
    let addr2 = addr.clone();
    addr.stop().unwrap();
    addr.await.unwrap();
    assert!(addr2.send(Store("password")).await.is_err());
}

#[test_log::test(tokio::test)]
async fn addr_stop() {
    let (event_loop, mut addr) = start(MyActor::default());
    tokio::spawn(event_loop);

    let addr2 = addr.clone();
    addr.stop().unwrap();

    addr2.await.unwrap();
    addr.await.unwrap();
}

#[test_log::test(tokio::test)]
async fn ctx_stop() {
    let (event_loop, addr) = start(MyActor::default());
    tokio::spawn(event_loop);

    let addr2 = addr.clone();
    addr.send(Stop).await.unwrap();

    addr2.await.unwrap();
    addr.await.unwrap();
}

#[test_log::test(tokio::test)]
async fn addr_stopped_after_stop() {
    let (event_loop, addr) = start(MyActor::default());
    tokio::spawn(event_loop);

    let addr2 = addr.clone();
    assert!(!addr2.stopped(), "addr2 should not be stopped");

    addr.send(Stop).await.unwrap();

    addr.await.unwrap();
    assert!(addr2.stopped(), "addr2 should be stopped");
}

#[test_log::test(tokio::test)]
async fn weak_addr_does_not_prolong_life() {
    let (event_loop, addr) = start(MyActor::default());
    let actor = tokio::spawn(event_loop);

    let weak_addr = WeakAddr::from(&addr);
    let weak_addr2 = addr.downgrade();
    weak_addr.upgrade().unwrap();
    weak_addr2.upgrade().unwrap();

    drop(addr);

    assert!(weak_addr.upgrade().is_none());
    assert!(weak_addr2.upgrade().is_none());
    actor.await.unwrap().unwrap();
}

#[test_log::test(tokio::test)]
async fn weak_caller_does_not_prolong_life() {
    let (event_loop, addr) = start(MyActor::default());
    let actor = tokio::spawn(event_loop);

    let weak_caller = addr.weak_caller::<Stop>();
    weak_caller.upgrade().unwrap();
    let weak_caller2 = addr.caller::<Stop>().downgrade();
    weak_caller2.upgrade().unwrap();

    drop(addr);

    assert!(weak_caller.upgrade().is_none());
    assert!(weak_caller2.upgrade().is_none());
    actor.await.unwrap().unwrap();
}

#[test_log::test(tokio::test)]
async fn weak_sender_does_not_prolong_life() {
    let (event_loop, addr) = start(MyActor::default());
    let actor = tokio::spawn(event_loop);

    let weak_sender = addr.weak_sender::<Stop>();
    weak_sender.upgrade().unwrap();
    let weak_sender2 = addr.sender::<Stop>().downgrade();
    weak_sender2.upgrade().unwrap();

    drop(addr);

    assert!(weak_sender.upgrade().is_none());
    assert!(weak_sender2.upgrade().is_none());
    actor.await.unwrap().unwrap();
}
