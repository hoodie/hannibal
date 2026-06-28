//! Unix signal handling via [`broadcast_signals`] and [`broadcast_signals_custom`].
//!
//! Actors subscribe to [`Signal`] via `ctx.subscribe::<Signal>()`.
use std::borrow::Borrow;

/// Re-exports [`async_signal::Signal`] as [`AsyncSignal`].
pub use async_signal::Signal as AsyncSignal;

use async_signal::Signals;

use crate::{
    Actor, Broker, Context, DynResult, Message, StreamHandler,
    actor::{service::Service, spawnable::StreamSpawnable as _},
};

/// Wraps an [`AsyncSignal`] for broadcasting via [`Broker<Signal>`](`crate::Broker`).
#[derive(Clone, Copy, Debug)]
pub struct Signal(pub AsyncSignal);

impl Message for Signal {
    type Response = ();
}

/// Service actor that listens for `SIGINT` and `SIGTERM` and broadcasts them as [`Signal`].
///
/// Use [`broadcast_signals`] to start this service. Actors can then subscribe to [`Signal`]
/// via `ctx.subscribe::<Signal>()`.
#[derive(Clone, Copy, Debug, Default)]
struct SignalService;
impl Actor for SignalService {}

impl Service for SignalService {
    async fn setup() -> DynResult<()> {
        let signals = Signals::new([async_signal::Signal::Int, async_signal::Signal::Term])?;
        SignalService.spawn_on_stream(signals).register().await?;

        Ok(())
    }
}

impl StreamHandler<std::io::Result<async_signal::Signal>> for SignalService {
    async fn handle(
        &mut self,
        _ctx: &mut Context<Self>,
        sig: std::io::Result<async_signal::Signal>,
    ) {
        let sig = match sig {
            Ok(sig) => sig,
            Err(error) => {
                log::warn!("Failed to read signal: {error}");
                return;
            }
        };

        if let Err(error) = Broker::<Signal>::publish(Signal(sig)).await {
            log::warn!("Failed to publish signal: {error}");
        }
    }

    async fn finished(&mut self, ctx: &mut Context<Self>) {
        if let Err(error) = ctx.stop() {
            log::warn!("Failed to stop actor: {error}");
        }
    }
}

/// Broadcasts `SIGINT` and `SIGTERM` to  all subscribers of [`Signal`].
///
/// Call this once at startup. For a custom set of signals, use [`broadcast_signals_custom`].
///
/// # Example
///
/// ```no_run
/// use hannibal::prelude::*;
/// use hannibal::{Signal, broadcast_signals};
///
/// #[derive(Default)]
/// struct MyActor {
///     count: u8,
/// }
///
/// impl Actor for MyActor {
///     async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
///         ctx.subscribe::<Signal>().await?;
///         Ok(())
///     }
/// }
///
/// impl Handler<Signal> for MyActor {
///     async fn handle(&mut self, ctx: &mut Context<Self>, _msg: Signal) {
///         self.count += 1;
///         if self.count == 3 {
///             let _ = ctx.stop();
///         }
///     }
/// }
///
/// # #[hannibal::main(flavor = "current_thread")]
/// # async fn main() -> DynResult<()> {
/// broadcast_signals().await?;
/// MyActor::default().spawn().await?;
/// # Ok(())
/// # }
/// ```
pub async fn broadcast_signals() -> DynResult<()> {
    SignalService::setup().await
}

/// Broadcasts a custom set of signals to all subscribers of [`Signal`].
///
/// For the common case of `SIGINT` and `SIGTERM`, use [`broadcast_signals`] instead.
///
/// # Example
///
/// ```no_run
/// use hannibal::prelude::*;
/// use hannibal::{Signal, broadcast_signals_custom};
/// use async_signal::Signal as OsSignal;
///
/// #[derive(Default)]
/// struct MyActor;
///
/// impl Actor for MyActor {
///     async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
///         ctx.subscribe::<Signal>().await?;
///         Ok(())
///     }
/// }
///
/// impl Handler<Signal> for MyActor {
///     async fn handle(&mut self, ctx: &mut Context<Self>, msg: Signal) {
///         match msg.0 {
///             OsSignal::Usr1 => println!("SIGUSR1 received"),
///             OsSignal::Usr2 => println!("SIGUSR2 received"),
///             _ => {}
///         }
///     }
/// }
///
/// # #[hannibal::main(flavor = "current_thread")]
/// # async fn main() -> DynResult<()> {
/// broadcast_signals_custom([OsSignal::Usr1, OsSignal::Usr2]).await?;
/// MyActor::default().spawn().await?;
/// # Ok(())
/// # }
/// ```
pub async fn broadcast_signals_custom<B>(signals: impl IntoIterator<Item = B>) -> DynResult<()>
where
    B: Borrow<async_signal::Signal>,
{
    let signals = Signals::new(signals)?;
    SignalService.spawn_on_stream(signals).register().await?;
    Ok(())
}
