<div align="center">

# Hannibal

<!-- Crates version -->
[![build](https://img.shields.io/github/workflow/status/hoodie/hannibal/CI)](https://github.com/hoodie/notify-rust/actions?query=workflow%3A"Continuous+Integration")
[![version](https://img.shields.io/crates/v/hannibal)](https://crates.io/crates/hannibal/)
[![downloads](https://img.shields.io/crates/d/hannibal.svg?style=flat-square)](https://crates.io/crates/hannibal)
[![docs.rs docs](https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square)](https://docs.rs/hannibal)
[![contributors](https://img.shields.io/github/contributors/hoodie/notify-rust)](https://github.com/hoodie/hannibal/graphs/contributors)
![maintenance](https://img.shields.io/maintenance/yes/2021)
[![license](https://img.shields.io/crates/l/hannibal.svg?style=flat)](https://crates.io/crates/hannibal/)


a small actor library
</div>

## Why
Credit where credit is due: Hannibal is a fork of the excellent [Xactor](https://github.com/sunli829/xactor) which unfortunately received very little interest by its original maintainer recently.
As of now there is no breaking change here, we could still merge xactor and hannibal again.
Please reach out [via an issue](https://github.com/hoodie/hannibal/issues/new) if you are interested.

## Documentation

- [GitHub repository](https://github.com/hoodie/hannibal)
- [Cargo package](https://crates.io/crates/hannibal)
- Minimum supported Rust version: 1.56 or later

## Features

- Async actors.
- Actor communication in a local context.
- Using Futures for asynchronous message handling.
- Typed messages (No `Any` type). Generic messages are allowed.

## Examples

```rust
use hannibal::*;

#[message(result = "String")]
struct ToUppercase(String);

struct MyActor;

impl Actor for MyActor {}

#[async_trait::async_trait]
impl Handler<ToUppercase> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: ToUppercase) -> String {
        msg.0.to_uppercase()
    }
}

#[hannibal::main]
async fn main() -> Result<()> {
    // Start actor and get its address
    let mut addr = MyActor.start().await?;

    // Send message `ToUppercase` to actor via addr
    let res = addr.call(ToUppercase("lowercase".to_string())).await?;
    assert_eq!(res, "LOWERCASE");
    Ok(())
}
```

## Installation

Hannibal requires [async-trait](https://github.com/dtolnay/async-trait) on userland.

With [cargo add][cargo-add] installed, run:

```sh
$ cargo add hannibal
$ cargo add async-trait
```

We also provide the [tokio](https://tokio.rs/) runtime instead of [async-std](https://async.rs/). To use it, you need to activate `runtime-tokio` and disable default features.

You can edit your `Cargo.toml` as follows:

```toml
hannibal = { version = "x.x.x", features = ["runtime-tokio"], default-features = false }
```

[cargo-add]: https://github.com/killercup/cargo-edit

## References

- [Actix](https://github.com/actix/actix)
- [Async-std](https://github.com/async-rs/async-std)
- [Tokio](https://github.com/tokio-rs/tokio)
- [Xactor](https://github.com/sunli829/xactor)
