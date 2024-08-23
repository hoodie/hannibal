# (Mini|Hanni)bal

A minimalistic reimplementation of the [Hannibal](https://lib.rs/hannibal) actor framework for Rust.

## Differences and Plans

- Typefree Context (not parameterized over concrete actor)
- Strong and Weak Senders and Callers (as in actix)
- Exchangeable Channel Implementation ((un)bound, std, futures, tokio, lets see)
- Exchangable Runtime (no compiletime feature, no hannibal::block_on())
- maybe no extra proc_macro derive for messages necessary
-


## TODO

- [ ] Async EventLoop
- [ ] Stopping actors + Notifying Addrs
- [ ] maybe impl SinkExt for Addr/Sender
- [ ] maybe impl async AND blocking sending
- [ ] impl Caller
- [ ] logging and console subscriber
