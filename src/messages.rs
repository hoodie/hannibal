//! # Standard Messages
//!
//! This module contains standard messages that are handled by any actor.
//!

use std::time::{Duration, Instant};

use super::{Actor, Handler, Message};

/// Ping the actor and receive the round-trip duration.
#[derive(Debug, Clone, Copy)]
pub struct Ping(Instant);
impl Message for Ping {
    type Response = Duration;
}

impl Default for Ping {
    fn default() -> Self {
        Ping(Instant::now())
    }
}

impl<A: Actor> Handler<Ping> for A {
    async fn handle(&mut self, _ctx: &mut crate::Context<Self>, Ping(sent_time): Ping) -> Duration {
        Instant::now() - sent_time
    }
}

/// Cause the Actor's Context to garbage-collect.
///
/// see [`Context::gc`](`crate::Context::gc`) for more information.
#[derive(Debug, Clone, Copy)]
pub struct GC;
impl Message for GC {
    type Response = ();
}

impl<A: Actor> Handler<GC> for A {
    async fn handle(&mut self, ctx: &mut crate::Context<Self>, _: GC) {
        log::trace!("Garbage Collecting");
        ctx.gc();
    }
}
