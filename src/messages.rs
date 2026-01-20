//! # Standard Messages
//!
//! This module contains standard messages that are handled by any actor.
//!

use std::time::{Duration, Instant};

use super::{Actor, Handler, Message};

/// **Message** Ping message containing the time it was sent.
#[derive(Debug, Clone, Copy)]
pub struct Ping(Instant);
impl Message for Ping {
    type Response = Pong;
}

impl Default for Ping {
    fn default() -> Self {
        Ping(Instant::now())
    }
}

/// **Response** Response to a Ping message containing the duration since the ping was sent.
#[derive(Debug, Clone, Copy)]
pub struct Pong(pub Duration);

impl<A: Actor> Handler<Ping> for A {
    async fn handle(&mut self, _ctx: &mut crate::Context<Self>, Ping(sent_time): Ping) -> Pong {
        Pong(std::time::Instant::now() - sent_time)
    }
}

/// **Message** Garbage Collect message.
///
/// This message can be used to trigger garbage collection within an actor.
#[derive(Debug, Clone, Copy)]
pub struct Gc;
impl Message for Gc {
    type Response = ();
}

impl<A: Actor> Handler<Gc> for A {
    async fn handle(&mut self, ctx: &mut crate::Context<Self>, _: Gc) {
        log::trace!("Garbage Collecting");
        ctx.gc();
    }
}
