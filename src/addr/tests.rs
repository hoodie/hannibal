#![allow(clippy::unwrap_used)]
use hannibal_derive::message;

use crate::{Context, DynResult, event_loop::EventLoop, prelude::Spawnable};

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
    let addr = MyActor::default().spawn();

    let addition = addr.call(Add(1, 2)).await.unwrap();
    assert_eq!(addition, 3);
}

#[test_log::test(tokio::test)]
async fn addr_send() {
    let owning_addr = MyActor::default().spawn_owning();
    owning_addr.send(Store("password")).await.unwrap();
    let actor = owning_addr.consume().await.unwrap();
    assert_eq!(actor.0, Some("password"))
}

#[test_log::test(tokio::test)]
async fn addr_send_err() {
    let addr = MyActor::default().spawn();
    let addr2 = addr.clone();
    addr.halt().await.unwrap();

    assert!(addr2.stopped());
    assert!(addr2.send(Store("password")).await.is_err());
}

#[test_log::test(tokio::test)]
async fn addr_stop() {
    let mut addr = MyActor::default().spawn();

    let addr2 = addr.clone();
    addr.stop().unwrap();

    addr2.await.unwrap();
    addr.await.unwrap();
}

#[test_log::test(tokio::test)]
async fn ctx_stop() {
    let addr = MyActor::default().spawn();

    let addr2 = addr.clone();
    addr.send(Stop).await.unwrap();

    addr2.await.unwrap();
    addr.await.unwrap();
}

#[test_log::test(tokio::test)]
async fn addr_stopped_after_stop() {
    let addr = MyActor::default().spawn();

    let addr2 = addr.clone();
    assert!(!addr2.stopped(), "addr2 should not be stopped");

    addr.send(Stop).await.unwrap();

    addr.await.unwrap();
    assert!(addr2.stopped(), "addr2 should be stopped");
}

#[test_log::test(tokio::test)]
async fn weak_addr_does_not_prolong_life() {
    let addr = MyActor::default().spawn();

    let weak_addr = WeakAddr::from(&addr);
    let weak_addr2 = addr.downgrade();
    weak_addr.upgrade().unwrap();
    weak_addr2.upgrade().unwrap();

    addr.halt().await.unwrap();

    assert!(weak_addr.upgrade().is_none());
    assert!(weak_addr2.upgrade().is_none());
}

#[test_log::test(tokio::test)]
async fn weak_caller_does_not_prolong_life() {
    let addr = MyActor::default().spawn();

    let weak_caller = addr.weak_caller::<Stop>();
    weak_caller.upgrade().unwrap();
    let weak_caller2 = addr.caller::<Stop>().downgrade();
    weak_caller2.upgrade().unwrap();

    addr.halt().await.unwrap();

    assert!(weak_caller.upgrade().is_none());
    assert!(weak_caller2.upgrade().is_none());
}

#[test_log::test(tokio::test)]
async fn weak_sender_does_not_prolong_life() {
    let addr = MyActor::default().spawn();

    let weak_sender = addr.weak_sender::<Stop>();
    weak_sender.upgrade().unwrap();
    let weak_sender2 = addr.sender::<Stop>().downgrade();
    weak_sender2.upgrade().unwrap();

    addr.halt().await.unwrap();

    assert!(weak_sender.upgrade().is_none());
    assert!(weak_sender2.upgrade().is_none());
}

mod test_error_variants {
    use super::*;
    use crate::{error::ActorError, prelude::Spawnable};

    #[test_log::test(tokio::test)]
    async fn mailbox_closed_on_send() {
        let addr = MyActor::default().spawn();
        let addr2 = addr.clone();
        addr.halt().await.unwrap();

        // Try to send a message - should fail with ChannelClosed
        assert!(addr2.send(Store("test")).await.is_err());
    }

    #[test_log::test(tokio::test)]
    async fn channel_closed_on_call() {
        let addr = MyActor::default().spawn();
        let addr2 = addr.clone();
        addr.halt().await.unwrap();

        assert!(addr2.call(Add(1, 2)).await.is_err());
    }

    #[test_log::test(tokio::test)]
    async fn channel_closed_on_try_send() {
        let addr = MyActor::default().spawn();
        let addr2 = addr.clone();
        addr.halt().await.unwrap();

        assert!(addr2.try_send(Store("test")).is_err());
    }

    #[test_log::test(tokio::test)]
    async fn channel_closed_on_ping() {
        let addr = MyActor::default().spawn();
        let addr2 = addr.clone();
        addr.halt().await.unwrap();

        assert!(addr2.ping().await.is_err());
    }

    #[test_log::test(tokio::test)]
    async fn channel_closed_via_sender() {
        let addr = MyActor::default().spawn();
        let sender = addr.sender::<Store>();
        addr.halt().await.unwrap();

        assert_eq!(
            sender.send(Store("test")).await,
            Err(ActorError::ChannelClosed)
        );
    }

    #[test_log::test(tokio::test)]
    async fn channel_closed_via_caller() {
        let addr = MyActor::default().spawn();
        let caller = addr.caller();
        addr.halt().await.unwrap();

        assert_eq!(caller.call(Add(1, 2)).await, Err(ActorError::ChannelClosed));
    }

    #[test_log::test(tokio::test)]
    async fn actor_already_stopped_on_stop() {
        let addr = MyActor::default().spawn();
        let mut addr2 = addr.clone();
        addr.halt().await.unwrap();

        assert_eq!(addr2.stop(), Err(ActorError::AlreadyStopped));
    }

    #[test_log::test(tokio::test)]
    async fn actor_already_stopped_from_context() {
        let addr = MyActor::default().spawn();
        let mut weak_addr = addr.downgrade();
        addr.halt().await.unwrap();

        assert_eq!(weak_addr.try_stop(), Err(ActorError::ActorDropped));
    }

    #[test_log::test(tokio::test)]
    async fn actor_dropped_on_weak_sender_upgrade_and_send() {
        let addr = MyActor::default().spawn();
        let weak_sender = addr.weak_sender();
        addr.halt().await.unwrap();

        assert_eq!(
            weak_sender.upgrade_and_send(Store("test")).await,
            Err(ActorError::ActorDropped)
        );
    }

    #[test_log::test(tokio::test)]
    async fn actor_dropped_on_weak_caller_upgrade_and_call() {
        let addr = MyActor::default().spawn();
        let weak_caller = addr.weak_caller();
        addr.halt().await.unwrap();

        assert_eq!(
            weak_caller.upgrade_and_call(Add(1, 2)).await,
            Err(ActorError::ActorDropped)
        );
    }

    #[test_log::test(tokio::test)]
    async fn actor_dropped_on_weak_addr_try_stop() {
        let addr = MyActor::default().spawn();
        let mut weak_addr = addr.downgrade();
        addr.halt().await.unwrap();

        assert_eq!(weak_addr.try_stop(), Err(ActorError::ActorDropped));
    }

    #[test_log::test(tokio::test)]
    async fn actor_dropped_on_weak_addr_try_halt() {
        let addr = MyActor::default().spawn();
        let mut weak_addr = addr.downgrade();
        addr.halt().await.unwrap();

        assert_eq!(weak_addr.try_halt().await, Err(ActorError::ActorDropped));
    }
}
