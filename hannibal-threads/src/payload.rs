use crate::{ActorResult, Context};

type Exec<A> =
    dyn for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> ActorResult<()> + Send + Sync + 'static;

pub enum Payload<A> {
    Exec(Box<Exec<A>>),
    Stop,
}

impl<A> Payload<A> {
    pub fn task<
        F: for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> ActorResult<()> + Send + Sync + 'static,
    >(
        f: F,
    ) -> Self {
        Self::Exec(Box::new(f))
    }
}
