<div align="center">

# Hannibal

<!-- Crates version -->

[![version](https://img.shields.io/crates/v/hannibal)](https://crates.io/crates/hannibal/)
[![downloads](https://img.shields.io/crates/d/hannibal.svg?style=flat-square)](https://crates.io/crates/hannibal)
[![docs.rs docs](https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square)](https://docs.rs/hannibal)
![maintenance](https://img.shields.io/maintenance/yes/2026)

a small actor library

</div>

## Motivation

In async Rust you find yourself often spawning tasks and instantiating channels to communicate between them.
This can be cumbersome in larger projects and become complicated once you want to support multiple message types.
You also end up handling the lifecycle of your tasks and channels manually again and again.
See [simple-channels-only.rs](examples/simple-channels-only.rs).

An actor is a task that encapsulates its own state and can receive messages.
You can pass around strong and weak Addresses to the concret actor type.

## Features

- feels like actix, but async
  - each actor runs in its own task
  - actors can be stopped or `.await`ed
- weak and strong addresses (by actor-type)
- weak and strong senders and callers (by message-type)
- using futures for asynchronous message handling.
- typed messages. Generic messages are allowed.

## Examples
### Addresses

Create a struct that becomes your actor and holds its state.
It can receive `Greet` and `Add` messages.

```rust
/// MyActor is a struct that will be turned into an actor
struct MyActor(&'static str);

#[message]
struct Greet(&'static str);

#[message(response = i32)]
struct Add(i32, i32);

impl Actor for MyActor {}

/// just print a greeting
impl Handler<Greet> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Greet) {
        println!(
            "[Actor {me}] Hello {you}, my name is {me}",
            me = self.0,
            you = msg.0,
        );
    }
}

/// handle the addition of two numbers and return the result
impl Handler<Add> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Add) -> i32 {
        msg.0 + msg.1
    }
}

// spawn the actor and get its address
let mut addr = MyActor("Caesar").spawn();

// send a message without a response
addr.send(Greet("Hannibal")).await.unwrap();

// expecting a response
let addition = addr.call(Add(1, 2)).await;

println!("The Actor Calculated: {:?}", addition);
```

> see [simple.rs](examples/simple.rs)

### Senders and Callers

You can also address actors by their message type, not their concrete type.
This is especially useful when you want to send a message to an actor without knowing its concrete type.
There might be multiple actors that can handle the same message type.

```rust
let sender = addr.sender::<Greet>();
let caller = addr.caller::<Add>();

// send a message without a response
sender.send(Greet("Hannibal")).await.unwrap();

// expecting a response
let addition = caller.call(Add(1, 2)).await;
println!("The Actor Calculated: {:?}", addition);
```

> see [simple.rs](examples/simple.rs)

### Handling Streams

Often you need to handle streams of messages, e.g. from a TCP connections or websockets.

```rust
#[derive(Default, Actor)]
struct FizzBuzzer(&'static str);

impl StreamHandler<i32> for FizzBuzzer {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32)
        match (msg % 3 == 0, msg % 5 == 0) {
            (true, true) => self.0 = "fizzbuzz",
            (true, false) => self.0 = "fizz",
            (false, true) => self.0 = "buzz",
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    // just imagine this is a websocket stream
    let num_stream = futures::stream::iter(1..30);
    let addr = hannibal::build(FizzBuzzer("Caesar"))
        .on_stream(num_stream)
        .spawn();

    addr.await.unwrap();
}
```
> see [stream.rs](examples/stream.rs)

### Services
Services are actors that can be accessed globally via a registry. This is useful for shared resources like databases or caches.

```rust
#[derive(Debug, Default, Actor, Service)]
struct StorageService {
    storage: HashMap<String, String>,
}

#[message]
struct Store(&'static str, &'static str);

#[message(response = Option<String>)]
struct Retrieve(&'static str);

impl Handler<Store> for StorageService {
    async fn handle(&mut self, _: &mut Context<Self>, Store(key, value): Store) {
        self.storage.insert(key.to_string(), value.to_string());
    }
}

impl Handler<Retrieve> for StorageService {
    async fn handle(&mut self, _: &mut Context<Self>, Retrieve(key): Retrieve) -> Option<String> {
        self.storage.get(key).cloned()
    }
}

// Setup the service
StorageService::setup().await.unwrap();

// Store a value
StorageService::from_registry().await
    .send(Store("password", "hello world")).await
    .unwrap();

// Retrieve the value
let result = StorageService::from_registry().await
    .call(Retrieve("password")) .await
    .unwrap();

println!("retrieved: {:?}", result);
```

> see [storage-service.rs](examples/storage-service.rs)

In this example, `StorageService` is a globally accessible service that stores key-value pairs. You can send messages to store and retrieve values from anywhere in your application.

### Brokers

Brokers are services that can be used to broadcast messages to multiple actors.

```rust
#[derive(Clone, Message)]
struct Topic1(u32);

#[derive(Debug, Default, Message)]
struct Subscribing(Vec<u32>);

impl Actor for Subscribing {
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        ctx.subscribe::<Topic1>().await?;
        Ok(())
    }
}

impl Handler<Topic1> for Subscribing {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Topic1) {
        self.0.push(msg.0);
    }
}

let subscriber1 = Subscribing::default().spawn_owning();
let subscriber2 = Subscribing::default().spawn_owning();

Broker::from_registry()
    .await
    .publish(Topic1(42))
    .await
    .unwrap();
Broker::publish(Topic1(23)).await.unwrap();

let value1 = subscriber1.stop_and_join().unwrap().await.unwrap();
let value2 = subscriber2.stop_and_join().unwrap().await.unwrap();
println!("Subscriber 1 received: {:?}", value1);
println!("Subscriber 2 received: {:?}", value2);
Ok(())
```

> see [broker.rs](examples/broker.rs)


### Intervals and Timers
### Builders
### Owning Addresses

## New since 0.12

Hannibal until v0.10 was a fork of [xactor](https://crates.io/crates/xactor).
Since 0.12 it is a complete, indicated by skipping versions 0.11 entirely.
The rewrite with the following features:

- Strong and Weak Senders and Callers (as in actix)
- Exchangeable Channel Implementation
  - included: bounded and unbounded
- Streams are Handled by launching an actor together with a stream.
  - Avoids extra tasks and simplifies the logic.
    The actor lives only as long as the stream.
- Actor trait only brings methods that you should implement (better "implement missing members" behavior)
- derive macros for `Actor`, `Service` and `Message`
- Owning Addresses
  - allows returning actor content after stop
- Builder

## Contribution

Any help in form of descriptive and friendly [issues](https://github.com/hoodie/notify-rust/issues) or comprehensive pull requests are welcome!

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in notify-rust by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

### Conventions

The Changelog of this library is generated from its commit log, there any commit message must conform with [https://www.conventionalcommits.org/en/v1.0.0/]. For simplicity you could make your commits with [convco](https://crates.io/crates/convco).
