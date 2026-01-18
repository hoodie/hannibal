<!-- <div align="center"> -->

# Hannibal

<!-- Crates version -->

[![Continuous Integration](https://github.com/hoodie/hannibal/actions/workflows/ci.yml/badge.svg)](https://github.com/hoodie/hannibal/actions/workflows/ci.yml)
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

- typed messages, with responses
- each actor runs in its own task
- async message exchange via addresses, senders and callers
- weak and strong addresses, senders and callers
- services and brokers
- intervals and timeouts
- actor hierarchies with children
- configurable channels

## Examples

### Addresses

Create a struct that becomes your actor and holds its state.

You can send messages to an actor without expecting a response.

```rust
/// MyActor is a struct that will be turned into an actor
#[derive(Actor)]
struct MyActor(&'static str);

#[message]
struct Greet(&'static str);

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

// spawn the actor and get its address
let mut addr = MyActor("Caesar").spawn();

// send a message without a response
addr.send(Greet("Hannibal")).await.unwrap();
```

You can also call the actor and get a response.

```rust
/// MyActor is a struct that will be turned into an actor
struct MyActor(&'static str);

#[message(response = i32)]
struct Add(i32, i32);

impl Actor for MyActor {}

/// handle the addition of two numbers and return the result
impl Handler<Add> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Add) -> i32 {
        msg.0 + msg.1
    }
}

// spawn the actor and get its address
let mut addr = MyActor("Caesar").spawn();

// expecting a response
let addition = addr.call(Add(1, 2)).await;

println!("The Actor Calculated: {:?}", addition);
```

> see [simple.rs](examples/simple.rs)

### Senders and Callers

You can also address actors by their message type, not their concrete type.
This is especially useful when you want to send a message to an actor without knowing its concrete type bacause
there might be multiple actors that can handle the same message type.

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

A `Sender` is a kind of `Arc` that allows you to send messages to an actor without knowing its concrete type. A `Caller` allows you to send messages to an actor and receive a response.

Both of them have weak equivalents too.

### Handling Streams

Often you need to handle streams of messages, e.g. from a TCP connections or websockets.
Actors can be spawned "on a stream". This way they are tightly coupled to the stream and will be notified once the stream is exhausted.

As of `v0.15.0` stream-handling actors **do not automatically stop when the stream is finished**.
If you want the actor to stop once the attached stream is exhausted, call `ctx.stop()` in `finished()`.

```rust
#[derive(Default, Actor)]
struct FizzBuzzer(&'static str);

impl StreamHandler<i32> for FizzBuzzer {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
        match (msg % 3 == 0, msg % 5 == 0) {
            (true, true) => self.0 = "fizzbuzz",
            (true, false) => self.0 = "fizz",
            (false, true) => self.0 = "buzz",
            _ => {}
        }
    }

    async fn finished(&mut self, ctx: &mut Context<Self>) {
        ctx.stop().unwrap();
    }
}

// just imagine this is a websocket stream
let num_stream = futures::stream::iter(1..30);
let addr = hannibal::setup_actor(FizzBuzzer::default())
    .on_stream(num_stream)
    .spawn();

// The actor terminates only if you stop it (e.g. in `finished()` above).
addr.await.unwrap();
```

> see [streams.rs](examples/streams.rs)

### Services

Services are actors that can be accessed globally via a registry.
This is useful for shared resources like databases or caches.
You do not need to have an `Addr` to the service,
you simply access it via the registry.
Services are started on demand.

```rust
#[derive(Debug, Default, Actor, Service)]
struct TimerService {}

#[message(response = String)]
struct Retrieve;

impl Handler<Retrieve> for TimerService {
    async fn handle(&mut self, _: &mut Context<Self>, Retrieve: Retrieve) -> String {
        format!("{:?}", std::time::Instant::now())
    }
}

let result = TimerService::from_registry()
    .await
    .call(Retrieve)
    .await
    .unwrap();

println!("retrieved: {:?}", result);

```

> see [time-service.rs](examples/time-service.rs) _(or [storage-service.rs](examples/storage-service.rs))_

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
```

> see [broker.rs](examples/broker.rs)

### Intervals and Timers

In order to do execute actions regularly or after a certain period of time,
actors can start intervals that send themselves messages.

```rs
struct MyActor(u8);

#[message]
struct Stop;

/// implement the `Actor` trait by hand
impl Actor for MyActor {
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        println!("[Actor] started");
        ctx.interval((), Duration::from_secs(1));
        ctx.delayed_send(|| Stop, Duration::from_secs(5));
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("[Actor] stopped");
    }
}

impl Handler<()> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: ()) {
        self.0 += 1;
        println!("[Actor] received interval message {}", self.0);
    }
}

impl Handler<Stop> for MyActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _msg: Stop) {
        println!("[Actor] received stop message");
        ctx.stop().unwrap();
    }
}

MyActor(0).spawn().await.unwrap();
```

> see [intervals.rs](examples/intervals.rs)

### Builders

You can configure certain aspects of an actor before starting it.
This includes

* should use bounded or unbounded channels?
* should it be restartable?
* should recreate itself from `Default` when being restarted
* should it enfore timeouts when handling messages?
* should it fail or continue if timeouts are exceeded?

> see the documentation of [build()](https://docs.rs/hannibal/latest/hannibal/fn.build.html) for more examples.

### Owning Addresses

If you should need to retain the ownership to the instance of your actor object you can hold an `OwningAddr`.
This yields the its content when stopped.
If you find a good usecase for this outside of testing, feel free to drop me a message to let me know 😜.

## Contribution

Any help in form of descriptive and friendly [issues](https://github.com/hoodie/hannibal/issues) or comprehensive pull requests are welcome!

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in hannibal by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

### Conventions

The Changelog of this library is generated from its commit log, there any commit message must conform with [https://www.conventionalcommits.org/en/v1.0.0/]. For simplicity you could make your commits with [convco](https://crates.io/crates/convco).
