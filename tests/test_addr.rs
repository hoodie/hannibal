use hannibal::*;

#[message(result = usize)]
struct AddUp(usize);

#[message]
struct Ping;

#[derive(Default)]
struct PingActor;
impl Actor for PingActor {}
impl Handler<Ping> for PingActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) {}
}

#[ctor::ctor]
fn init_color_backtrace() {
    color_backtrace::install();
}

#[test]
fn stop_addr() {
    async fn main() -> hannibal::Result<()> {
        let mut addr = hannibal::Actor::start(PingActor).await?;
        let addr2 = addr.clone();

        assert!(!addr.stopped(), "expected addr not to be stopped");
        assert!(!addr2.stopped(), "expected addr2 not to be stopped");

        addr.stop(None).unwrap();
        assert!(!addr.stopped());
        assert!(!addr2.stopped());
        addr.wait_for_stop().await;

        assert!(addr2.stopped(), "expected addr2 to be stopped");

        Ok(())
    }

    hannibal::block_on(main()).unwrap();
}

#[test]
fn await_stop_addr() {
    async fn main() -> hannibal::Result<()> {
        let mut addr = hannibal::Actor::start(PingActor).await?;
        let addr2 = addr.clone();

        assert!(!addr.stopped(), "expected addr not to be stopped");
        assert!(!addr2.stopped(), "expected addr2 not to be stopped");

        addr.stop(None).unwrap();
        addr.await.unwrap();

        assert!(addr2.stopped(), "expected addr2 to be stopped");

        Ok(())
    }

    hannibal::block_on(main()).unwrap();
}
mod callers {
    use super::*;

    #[derive(Default)]
    struct AddUpActor {
        sum: usize,
    }

    impl Actor for AddUpActor {}

    impl Handler<AddUp> for AddUpActor {
        async fn handle(&mut self, _ctx: &mut Context<Self>, AddUp(diff): AddUp) -> usize {
            self.sum += diff;
            self.sum
        }
    }

    #[message(result = usize)]
    struct SleepAndAddUp(std::time::Duration, usize);
    impl Handler<SleepAndAddUp> for AddUpActor {
        async fn handle(
            &mut self,
            _ctx: &mut Context<Self>,
            SleepAndAddUp(dur, diff): SleepAndAddUp,
        ) -> usize {
            hannibal::sleep(dur).await;
            self.sum += diff;
            self.sum
        }
    }

    #[test]
    fn caller_sends_message_and_returns_response() {
        hannibal::block_on(async {
            let addr = AddUpActor { sum: 1 }.start_bounded(1).await.unwrap();

            let caller = addr.caller::<AddUp>();

            assert_eq!(caller.call(AddUp(1)).await.unwrap(), 2);
            assert_eq!(caller.call(AddUp(1)).await.unwrap(), 3);
            assert_eq!(caller.call(AddUp(1)).await.unwrap(), 4);
        });
    }

    /// this is more of a lifecycle test
    /// at some point it might be nice to have parallel handling of messages
    #[test]
    fn calls_wait_in_line() {
        hannibal::block_on(async {
            let addr = AddUpActor { sum: 0 }.start().await.unwrap();

            let caller = addr.caller::<SleepAndAddUp>();

            let results = futures::try_join!(
                caller.call(SleepAndAddUp(std::time::Duration::from_millis(60), 1)),
                caller.call(SleepAndAddUp(std::time::Duration::from_millis(20), 10)),
                caller.call(SleepAndAddUp(std::time::Duration::from_millis(0), 100))
            )
            .unwrap();
            assert_eq!(results, (1, 11, 111))
        });
    }

    #[test]
    fn can_be_cloned() {
        async fn main() -> hannibal::Result<()> {
            // start new actor
            let addr = PingActor.start().await?;

            let caller: Caller<Ping> = addr.caller();
            let caller2 = &Clone::clone(&caller);
            let _caller_is_send: &dyn Send = &Clone::clone(&caller);
            let _caller_is_sync: &dyn Sync = &Clone::clone(&caller);

            caller.call(Ping).await?;
            caller2.call(Ping).await?;

            Ok(())
        }
        hannibal::block_on(main()).unwrap();
    }

    mod weak {
        use super::*;

        #[test]
        fn do_not_sustain_addr() {
            hannibal::block_on(async {
                let addr = AddUpActor { sum: 10 }.start().await.unwrap();

                let caller: WeakCaller<AddUp> = addr.weak_caller();
                let caller2 = caller.clone();

                assert!(caller.upgrade().is_some());

                assert_eq!(caller.try_call(AddUp(10)).await.unwrap(), 20);
                assert_eq!(caller2.try_call(AddUp(10)).await.unwrap(), 30);

                drop(addr);

                assert!(caller.upgrade().is_none());
                assert!(caller2.upgrade().is_none());
            })
        }

        #[test]
        fn are_bound_by_the_addr_not_their_strong_counterpart() {
            hannibal::block_on(async {
                let addr = AddUpActor { sum: 10 }.start().await.unwrap();
                let caller = addr.caller::<AddUp>();
                let weak_caller = caller.downgrade();
                drop(caller);
                assert!(weak_caller.upgrade().is_some());
                drop(addr);
                assert!(weak_caller.upgrade().is_none());
            })
        }
    }

    #[test]
    #[ignore = "this is flaky, send failing is not guaranteed if the actor was stopped"]
    fn do_not_upgrade_if_the_actor_was_stopped() {
        hannibal::block_on(async {
            let mut addr = AddUpActor::start_default().await.unwrap();
            let sender = addr.caller::<AddUp>();

            let weak_sender = sender.downgrade();

            addr.stop(None).unwrap();
            addr.wait_for_stop().await;

            assert!(
                weak_sender.upgrade().is_some(),
                "expecting weak_sender to still upgrade"
            );

            assert!(
                weak_sender.upgrade().unwrap().call(AddUp(1)).await.is_err(),
                "weak_sender should not be able to send"
            );

            drop(sender);
            assert!(
                weak_sender.upgrade().is_none(),
                "expecting weak_sender to not upgrade"
            );
        })
    }
}

mod senders {
    use super::*;

    #[test]
    fn can_be_cloned() {
        hannibal::block_on(async {
            // start new actor
            let addr = PingActor.start().await.unwrap();

            let sender: Sender<Ping> = addr.sender();
            let sender2 = &Clone::clone(&sender);
            let _sender_is_send: &dyn Send = &Clone::clone(&sender);
            let _sender_is_sync: &dyn Sync = &Clone::clone(&sender);

            addr.send(Ping).unwrap();
            sender.send(Ping).unwrap();
            sender2.send(Ping).unwrap();
        })
    }

    mod weak {
        use super::*;

        #[test]
        fn do_not_sustain_addr() {
            hannibal::block_on(async {
                let addr = PingActor.start().await.unwrap();

                let sender: WeakSender<Ping> = addr.weak_sender();

                assert!(sender.upgrade().is_some());

                assert!(sender.upgrade().unwrap().send(Ping).is_ok());

                drop(addr);

                assert!(sender.upgrade().is_none());
            })
        }

        #[test]
        fn are_bound_by_the_addr_not_their_strong_counterpart() {
            hannibal::block_on(async {
                let addr = PingActor.start().await.unwrap();
                let sender = addr.sender::<Ping>();
                let weak_sender = sender.downgrade();
                drop(sender);
                assert!(weak_sender.upgrade().is_some());
                drop(addr);
                assert!(weak_sender.upgrade().is_none());
            })
        }

        #[test]
        #[ignore = "this is flaky, send failing is not guaranteed if the actor was stopped"]
        fn do_not_upgrade_if_the_actor_was_stopped() {
            hannibal::block_on(async {
                let mut addr = PingActor.start().await.unwrap();
                let sender = addr.sender::<Ping>();

                let weak_sender = sender.downgrade();

                addr.stop(None).unwrap();
                addr.wait_for_stop().await;

                assert!(
                    weak_sender.upgrade().is_some(),
                    "expecting weak_sender to still upgrade"
                );

                assert!(
                    weak_sender.upgrade().unwrap().send(Ping).is_err(),
                    "weak_sender should not be able to send"
                );

                drop(sender);
                assert!(
                    weak_sender.upgrade().is_none(),
                    "expecting weak_sender to not upgrade"
                );
            })
        }
    }
}
