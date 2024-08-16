use std::future::Future;

use crate::{addr::Message, context::Context, error::Result};

pub trait Actor: Sized + Send + 'static {
    #[allow(unused)]
    fn started(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    #[allow(unused)]
    fn stopped(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async {}
    }
}

pub trait Handler<M: Message>: Actor
where
    Self: Sized,
{
    fn handle(
        &mut self,
        ctx: &mut Context<Self>,
        msg: M,
    ) -> impl futures::Future<Output = M::Result> + Send;
}
