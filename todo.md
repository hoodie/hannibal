
## TODO

- [x] Async EventLoop
- [x] Stopping actors + Notifying Addrs
- [x] environment builder
  - [x] return actor after stop
- [x] impl Caller/Sender
  - same old, same old
  - [x] stream handling
- [x] service
  - [x] restarting actors
  - [x] special addr that only allows restarting
- [x] manage child actors
- [x] broker
  - [ ] look into why there should be a thread local broker
- [x] deadlines (called timeouts)
- [x] intervals and timeouts
- [x] register children
- [ ] stream handling service/broker
   - [ ] allow a service that handles e.g. [signals](https://docs.rs/async-signals/latest/async_signals/struct.Signals.html)
   - [ ] (optional) have utility services already?
   - [ ] SUPPORT restarting stream handlers
- [x] logging and console subscriber
- [ ] stop reason
- [x] owning addr
   - returns actor again after stop
- [x] builder to configure
  - channel capacity
  - restart strategy

## Stretch Goals
- [x] can we select!() ?
  - yes, we do that for streams now
- [ ] maybe impl SinkExt for Addr/Sender
- [x] maybe impl async AND blocking sending

