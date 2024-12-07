use std::future::Future;

use crate::{Actor, Context, Message};

pub trait Handler<M: Message>: Actor {
    fn handle(
        &mut self,
        ctx: &mut Context<Self>,
        msg: M,
    ) -> impl futures::Future<Output = M::Response> + Send;
}

pub trait StreamHandler<M: 'static>: Actor {
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
