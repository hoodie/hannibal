# (Mini|Hanni)bal

A minimalistic reimplementation of the [Hannibal](https://lib.rs/hannibal) actor framework for Rust.

## Differences and Plans

- ~Typefree Context (not parameterized over concrete actor)~
- Strong and Weak Senders and Callers (as in actix)
- Exchangeable Channel Implementation ((un)bound, std, futures, tokio, lets see)
- Exchangable Runtime (no compiletime feature, no hannibal::block_on())
- maybe no extra proc_macro derive for messages necessary
- Streams are Handled by launching an actor together with a stream. This avoids extra tasks and simplifies the logic.
  The actor lives only as long as the stream.

## TODO

- [x] Async EventLoop
- [x] Stopping actors + Notifying Addrs
- [x] environment builder
  - [x] return actor after stop
- [x] impl Caller/Sender
  - same old, same old
  - [x] stream handling
    - [ ] attach api (nope)
- [x] service
  - [x] restarting actors
  - [x] special addr that only allows restarting
- [x] manage child actors
- [ ] broker
  - look into why there should be a thread local broker
- [ ] intervals and timeouts
  - injectable spawner/sleeper or at least separte impls
  - [ ] implement via child actors and special interval/timeout actor utility
- [ ] stream handling service/broker
   - [ ] allow a service that handles e.g. [signals](https://docs.rs/async-signals/latest/async_signals/struct.Signals.html)
   - [ ] (optional) have utility services already?
- [ ] logging and console subscriber
- [ ] test with rstest
- [ ] stop reason
- [ ] builder to configure
  - channel capacity
  - restart strategy


## Stretch Goals
- [x] can we select!() ?
  - yes, we do that for streams now
- [ ] maybe impl SinkExt for Addr/Sender
- [ ] maybe impl async AND blocking sending
