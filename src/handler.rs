use crate::{Actor, Context, Message};

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
