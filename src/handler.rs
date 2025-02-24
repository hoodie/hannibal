use std::future::Future;

use crate::{Actor, Context, Message};

/// An actor should implement this trait if it wants to handle messages.
pub trait Handler<M: Message>: Actor {
    /// Handle a message.
    ///
    /// Messages must implement the [`Message`] trait and can be sent via an [`Addr`](`crate::Addr`), [`Sender`](`crate::Sender`) or [`Caller`](`crate::Caller`).
    /// You must return the associated [response](`Message::Response`) type of the message.
    fn handle(
        &mut self,
        ctx: &mut Context<Self>,
        msg: M,
    ) -> impl futures::Future<Output = M::Response> + Send;
}

pub trait StreamHandler<M: 'static>: Actor {
    /// Handle a message from a stream.
    fn handle(
        &mut self,
        ctx: &mut Context<Self>,
        msg: M,
    ) -> impl futures::Future<Output = ()> + Send;

    #[allow(unused)]
    fn finished(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async {}
    }
}
