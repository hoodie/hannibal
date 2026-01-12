#![allow(clippy::unwrap_used)]
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use crate::runtime::sleep;

use super::*;
use crate::prelude::*;

#[derive(Debug)]
struct IntervalActor {
    running: Arc<AtomicBool>,
}

impl Actor for IntervalActor {
    async fn stopped(&mut self, _: &mut Context<Self>) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Handler<()> for IntervalActor {
    async fn handle(&mut self, _: &mut Context<Self>, _cmd: ()) {
        self.running.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn stopped_when_actor_stopped() {
    let flag = Arc::new(AtomicBool::new(false));
    let addr = IntervalActor {
        running: Arc::clone(&flag),
    }
    .spawn();
    sleep(Duration::from_millis(300)).await;
    addr.halt().await.unwrap();
    sleep(Duration::from_millis(300)).await;
    assert!(
        !flag.load(Ordering::SeqCst),
        "Handler should not be called after actor is stopped"
    );
}

mod interval_order {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn handlers_never_overlap() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, AtomicU32};

        let handler_running = Arc::new(AtomicBool::new(false));
        let overlap_detected = Arc::new(AtomicBool::new(false));
        let handler_count = Arc::new(AtomicU32::new(0));

        struct NoOverlapActor {
            handler_running: Arc<AtomicBool>,
            overlap_detected: Arc<AtomicBool>,
            handler_count: Arc<AtomicU32>,
        }

        #[derive(Clone)]
        #[message]
        struct SlowMessage;

        impl Handler<SlowMessage> for NoOverlapActor {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _: SlowMessage) {
                let was_running = self.handler_running.swap(true, Ordering::SeqCst);

                if was_running {
                    self.overlap_detected.store(true, Ordering::SeqCst);
                    panic!(
                        "Handler overlap detected! A handler started while another was still running."
                    );
                }

                self.handler_count.fetch_add(1, Ordering::SeqCst);

                sleep(Duration::from_millis(100)).await;

                self.handler_running.store(false, Ordering::SeqCst);
            }
        }

        impl Actor for NoOverlapActor {
            async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
                ctx.interval_with(|| SlowMessage, Duration::from_millis(20));
                Ok(())
            }
        }

        let addr = crate::setup_actor(NoOverlapActor {
            handler_running: Arc::clone(&handler_running),
            overlap_detected: Arc::clone(&overlap_detected),
            handler_count: Arc::clone(&handler_count),
        })
        .bounded(10)
        .spawn();

        sleep(Duration::from_millis(300)).await;
        addr.halt().await.unwrap();

        assert!(
            !overlap_detected.load(Ordering::SeqCst),
            "Handlers should never overlap"
        );

        let count = handler_count.load(Ordering::SeqCst);
        assert!(
            count >= 2,
            "At least 2 handlers should have executed (got {count})",
        );

        assert!(
            !handler_running.load(Ordering::SeqCst),
            "No handler should be running after halt"
        );
    }
}

mod interval_with {
    use super::*;
    use crate::{TaskHandle, actor::spawnable::Spawnable};

    #[derive(Debug)]
    struct IntervalWithActor {
        running: Arc<AtomicBool>,
        interval: Option<TaskHandle>,
    }

    impl Actor for IntervalWithActor {
        async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
            self.interval
                .replace(ctx.interval_with(|| (), Duration::from_millis(100)));
            Ok(())
        }
        async fn stopped(&mut self, _: &mut Context<Self>) {
            self.running.store(false, Ordering::SeqCst);
        }
    }

    impl Handler<()> for IntervalWithActor {
        async fn handle(&mut self, _: &mut Context<Self>, _: ()) {
            self.running.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn stopped_by_task_handle() {
        let running = Arc::new(AtomicBool::new(false));
        let addr = IntervalWithActor {
            running: Arc::clone(&running),
            interval: None,
        }
        .spawn();
        sleep(Duration::from_millis(300)).await;

        #[derive(hannibal_derive::Message)]
        struct StopInterval;
        impl Handler<StopInterval> for IntervalWithActor {
            async fn handle(
                &mut self,
                ctx: &mut Context<Self>,
                _msg: StopInterval,
            ) -> <StopInterval as Message>::Response {
                self.running.store(false, Ordering::SeqCst);
                if let Some(interval) = self.interval {
                    ctx.stop_task(interval)
                }
            }
        }

        addr.send(StopInterval).await.unwrap();
        sleep(Duration::from_millis(300)).await;
        assert!(
            !running.load(Ordering::SeqCst),
            "Handler should not be called after actor is stopped"
        );
    }

    #[tokio::test]
    async fn stopped_when_actor_stopped() {
        let running = Arc::new(AtomicBool::new(false));
        let addr = IntervalWithActor {
            running: Arc::clone(&running),
            interval: None,
        }
        .spawn();
        sleep(Duration::from_millis(300)).await;
        addr.halt().await.unwrap();
        sleep(Duration::from_millis(300)).await;
        assert!(
            !running.load(Ordering::SeqCst),
            "Handler should not be called after actor is stopped"
        );
    }
}

mod delayed_send {
    use super::*;

    #[derive(Debug)]
    struct DelayedSendActor {
        running: Arc<AtomicBool>,
    }

    impl Actor for DelayedSendActor {
        async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
            ctx.delayed_send(|| (), Duration::from_millis(100));
            Ok(())
        }
        async fn stopped(&mut self, _: &mut Context<Self>) {
            self.running.store(false, Ordering::SeqCst);
        }
    }

    impl Handler<()> for DelayedSendActor {
        async fn handle(&mut self, _: &mut Context<Self>, _: ()) {
            self.running.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn stopped_when_actor_stopped() {
        let running = Arc::new(AtomicBool::new(false));
        let addr = DelayedSendActor {
            running: Arc::clone(&running),
        }
        .spawn();
        sleep(Duration::from_millis(300)).await;
        addr.halt().await.unwrap();
        sleep(Duration::from_millis(300)).await;
        assert!(
            !running.load(Ordering::SeqCst),
            "Handler should not be called after actor is stopped"
        );
    }
}
