use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::FutureExt;

use super::Addr;
use crate::Result;

impl<A> std::future::Future for Addr<A> {
    type Output = Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(mut rx_exit) = this.rx_exit.take() {
            let poll = rx_exit.poll_unpin(cx);
            this.rx_exit = Some(rx_exit);
            poll.map(|p| p.map_err(Into::into))
        } else {
            Poll::Ready(Ok(()))
        }
    }
}
