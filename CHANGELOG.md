# Changelog

### [v0.12.1](https://github.com/hoodie/hannibal/compare/v0.12.0...v0.12.1) (2025-05-03)

#### Fixes

* default to tokio_runtime
([10405df](https://github.com/hoodie/hannibal/commit/10405df3bad7b8d1ca12028daa24fa834b7981f9))
* correct categories
([50af237](https://github.com/hoodie/hannibal/commit/50af23737f1b300ceff8ec751c58c39c13b62295))

## v0.12.0 (2025-04-27)

### Features

* support for sending messages to child actors
([1e61bf3](https://github.com/hoodie/hannibal/commit/1e61bf318fecc620759e6d79daa013d66991cdda))
* support for sending messages to child actors
([fd9f3f4](https://github.com/hoodie/hannibal/commit/fd9f3f42f7e01deae3b5b52236f5d66576959e01))
* support smol runtime
([e4f5b1f](https://github.com/hoodie/hannibal/commit/e4f5b1f19fddf4345d6f7de3c50dc45ad02a7718))
* replace concrete tokio::main with own agnostic main attribute
([6dd6758](https://github.com/hoodie/hannibal/commit/6dd67587c3264e217d6eed95ec9bed8ccc766a45))
* add delayed_exec() helper to context
([95d4550](https://github.com/hoodie/hannibal/commit/95d455006f8b11c9d21cfc06dfd575288eeff33d))
* add try_stop() and try_halt() helpers to WeakAddr
([84674ee](https://github.com/hoodie/hannibal/commit/84674eed1421b673b06cc1756213dbb91bd0f7a9))
* add address to context
([0a54839](https://github.com/hoodie/hannibal/commit/0a5483935ce6264412a9b449370d7715df5c3c1f))
* rename to hannibal
([88d5b8f](https://github.com/hoodie/hannibal/commit/88d5b8fb3d60cc034430c07d39ef563f68f3e80c))
* add convenience methods to builders
([bc3a15d](https://github.com/hoodie/hannibal/commit/bc3a15deda0ec864dcad91571e677bf0cb639284))
* add .call(), .send() and .ping() to OwningAddr
([5739810](https://github.com/hoodie/hannibal/commit/5739810c1c32a854f3f569e08649c9ec80290f70))
* add .consume() to OwningAddr
([46f06b1](https://github.com/hoodie/hannibal/commit/46f06b197261edc14701bc74c4f5fc59001d89df))
* add .ping() to Addr
([d281452](https://github.com/hoodie/hannibal/commit/d281452c01e25d4c5afd0b0fa7b718bc329a89ea))
* implement macros
([e5ce024](https://github.com/hoodie/hannibal/commit/e5ce0248828c1d24c352f63b5800831dcdd4639a))
* add actor names and logging
([06909fd](https://github.com/hoodie/hannibal/commit/06909fd5c2ae800dbfb464840738430729821961))
* add broker
([61694d4](https://github.com/hoodie/hannibal/commit/61694d42b113fe81a58338f67d2dbcbe352ebfa6))
* expose OwningAddr
([730c39a](https://github.com/hoodie/hannibal/commit/730c39a6d8a1a3ae1e0d5f1d06df1489a7f4050a))
* add intervals and delayed_send
([ade415e](https://github.com/hoodie/hannibal/commit/ade415e6e81544efe060830d9288b556811e9b2d))
* add timeouting
([11c71b8](https://github.com/hoodie/hannibal/commit/11c71b86b7b1edf3477b9628870bf24963d4eb21))
* add OwningAddr
([e440a84](https://github.com/hoodie/hannibal/commit/e440a84146d09e079277ef9c36e3df1beb76470f))
* fail registering service if there is already one running
([83b7b7e](https://github.com/hoodie/hannibal/commit/83b7b7ea5a29a749c3efe5847c0229d7aa6c7bcb))
* add builder
([afda883](https://github.com/hoodie/hannibal/commit/afda88391c5d399c611c3f967037eb38d0b212af))
* add NonRestartable
([757b1ed](https://github.com/hoodie/hannibal/commit/757b1ed9f76deefcdeb9ccb35e9f95d8b98d96e9))
* add holding children in context
([6d35f6b](https://github.com/hoodie/hannibal/commit/6d35f6b5f73837de7ad82c0ce1823d5d79410d55))
* make Addr::restart() dependent on the Actor implementing a marker trait
([99bcab5](https://github.com/hoodie/hannibal/commit/99bcab503f1e2291345cf5706dd347a2d2cafada))
* add SpawnableService trait
([64d9de6](https://github.com/hoodie/hannibal/commit/64d9de67dee1534f280014918c22d05d61e35deb))
* call finished on streamhandler when stream is done
([754ebfa](https://github.com/hoodie/hannibal/commit/754ebfa4d97312616fa88fb92c727d1807e20f67))
* add service trait
([7f9a979](https://github.com/hoodie/hannibal/commit/7f9a9798426774cc84eb0bf851c3914285eae0fb))
* add Spawnable trait to start actors
([9ea6e79](https://github.com/hoodie/hannibal/commit/9ea6e797e7a36a31302fc3941b06c84fd53e3ff2))
* add restart strategies for actors
([a53c1b8](https://github.com/hoodie/hannibal/commit/a53c1b84a8afa09ca5fe15aec1bd948a1c2c194d))
* add distinct StreamHandler trait
([a7c57c0](https://github.com/hoodie/hannibal/commit/a7c57c0715bd0c9304ee51262951da372acbd2e7))
* stream handling
([355d636](https://github.com/hoodie/hannibal/commit/355d636c8951ed448af627f3029100f875fa6a8d))
* add helper methods to create Senders and Callers
([a195950](https://github.com/hoodie/hannibal/commit/a195950fe5e938af70e662149e0989296f46f7ee))
* add WeakAddr
([1227e63](https://github.com/hoodie/hannibal/commit/1227e63cef8cadde42fc32777d0fe4e022eb5b01))
* add callers and senders weak and strong
([374e593](https://github.com/hoodie/hannibal/commit/374e59374c09590c82bc830d9d3db8b5033b7f52))
* support actor mutability
([b46a4c1](https://github.com/hoodie/hannibal/commit/b46a4c1e74e42b96e4caa029ec5be018e0b60e72))
* up and downgrading
([6c4133e](https://github.com/hoodie/hannibal/commit/6c4133e282a5876ca937788b1edd825718bbeff1))
* minimalist actor framework in 150 lines
([5160ce0](https://github.com/hoodie/hannibal/commit/5160ce02dd8e0599f99d29d3782cd4f95243d0ab))

### Fixes

* make send queue safe
([6879f98](https://github.com/hoodie/hannibal/commit/6879f98de52e889d7db46f9c8e020e26a0191f20))
