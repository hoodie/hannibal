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

/// An actor should implement this trait if it wants to handle messages from a stream.
///
/// ## Example:
/// ```
/// # use hannibal::prelude::*;
/// #[derive(Default, Actor)]
/// struct FizzBuzzer();
///
/// impl StreamHandler<i32> for FizzBuzzer {
///     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
///         match (msg % 3 == 0, msg % 5 == 0) {
///             (true, true)  => println!("fizzbuzz"),
///             (true, false) => println!("fizz"),
///             (false, true) => println!("buzz"),
///             _ => {}
///         }
///     }
///
///     async fn finished(&mut self, ctx: &mut Context<Self>) {
///         ctx.stop().unwrap();
///     }
/// }
///
/// # #[hannibal::main]
/// # async fn main() {
/// let num_stream = futures::stream::iter(1..30);
/// let addr = hannibal::build(FizzBuzzer::default())
///     .on_stream(num_stream)
///     .spawn();
///
/// addr.await.unwrap();
/// # }
/// ```
pub trait StreamHandler<M: 'static>: Actor {
    /// Handle a message from a stream.
    ///
    /// Messages received through this do not necessarily need to implement the `Message` trait.
    fn handle(
        &mut self,
        ctx: &mut Context<Self>,
        msg: M,
    ) -> impl futures::Future<Output = ()> + Send;

    /// Handle the end of the stream.
    ///
    /// This method is called when the stream is finished.
    /// You can use this method to perform any cleanup or finalization tasks.
    #[allow(unused)]
    fn finished(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async {}
    }
}
