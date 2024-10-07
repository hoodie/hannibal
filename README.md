# (Mini|Hanni)bal

A minimalistic reimplementation of the [Hannibal](https://lib.rs/hannibal) actor framework for Rust.

## Differences and Plans

- ~Typefree Context (not parameterized over concrete actor)~
- Strong and Weak Senders and Callers (as in actix)
- Exchangeable Channel Implementation ((un)bound, std, futures, tokio, lets see)
- Exchangable Runtime (no compiletime feature, no hannibal::block_on())
- maybe no extra proc_macro derive for messages necessary
-

## TODO

- [x] Async EventLoop
- [x] Stopping actors + Notifying Addrs
- [x] environment builder
  - [x] return actor after stop
- [x] impl Caller/Sender
  - same old, same old
  - [ ] stream handling
    - attach api
- [ ] service
  - [ ] restarting actors
- [ ] broker
  - look into why there should be a thread local broker
- [ ] intervals and timeouts
  - injectable spawner/sleeper or at least separte impls
- [ ] stream handling service/broker
   - [ ] allow a service that handles e.g. [signals](https://docs.rs/async-signals/latest/async_signals/struct.Signals.html)
   - [ ] (optional) have utility services already?
- [ ] logging and console subscriber
- [ ] test with rstest
- [ ] stop reason

## Stretch Goals
- [ ] can we select!() ?
- [ ] maybe impl SinkExt for Addr/Sender
- [ ] maybe impl async AND blocking sending
