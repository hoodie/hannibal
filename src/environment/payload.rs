use std::{future::Future, pin::Pin};

use crate::{Actor, Context};

type TaskFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

type TaskFn<A> =
    Box<dyn for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> TaskFuture<'a> + Send + 'static>;

pub(crate) enum Payload<A> {
    Task(TaskFn<A>),
    Stop,
    Restart,
}

impl<A: Actor> Payload<A> {
    pub fn task<F>(f: F) -> Self
    where
        F: for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> TaskFuture<'a> + Send + 'static,
    {
        Self::Task(Box::new(f))
    }
}
