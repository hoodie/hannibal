#[cfg(not(feature = "async_channel"))]
mod fc;

#[cfg(feature = "async_channel")]
mod ac;

#[cfg(not(feature = "async_channel"))]
pub(crate) use fc::{Channel, Rx, Tx, WeakTx};

#[cfg(feature = "async_channel")]
pub(crate) use ac::{Channel, Rx, Tx, WeakTx};
