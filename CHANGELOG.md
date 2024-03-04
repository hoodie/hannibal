# Changelog

## [v0.9.0](https://github.com/hoodie/hannibal/compare/v0.8.3...v0.9.0) (2024-03-04)

### ⚠ BREAKING CHANGE

* Senders and Callers must now be cloned


### Features

* switch from tokio-sync to shared oneshot
([7c29acd](https://github.com/hoodie/hannibal/commit/7c29acd75187362ab5224fb50449a83759ba4f15))

### Fixes

* Sender::Send returns Error if Actor was dropped
([9335561](https://github.com/hoodie/hannibal/commit/93355612270c186c65dedbd5ffe54e762f6d97f8))

### [v0.8.3](https://github.com/hoodie/hannibal/compare/v0.8.2...v0.8.3) (2022-03-03)

#### Features

* annotate actors with names for console
([8f6617d](https://github.com/hoodie/hannibal/commit/8f6617d1f61b4a67279680648a7f2e1688244f56))

### [v0.8.2](https://github.com/hoodie/hannibal/compare/v0.8.1...v0.8.2) (2022-02-28)

#### Features

* add upgrade_send method to WeakAddr
([7ba5b27](https://github.com/hoodie/hannibal/commit/7ba5b273d44d68c4649ddcd3a4b1b977b47d334f))

### [v0.8.1](https://github.com/hoodie/hannibal/compare/v0.8.0...v0.8.1) (2022-02-08)

#### Features

* add non-exhaustive debug-print for Addr and WeakAddr
([090913d](https://github.com/hoodie/hannibal/commit/090913d7739a593284c21961d9356ab59226449e))

## [v0.8.0](https://github.com/hoodie/hannibal/compare/v0.7.11...v0.8.0) (2022-01-30)

### Features

* make Sender and Caller Sync
([2725980](https://github.com/hoodie/hannibal/commit/2725980c41e51ceffd3593f37fbe4dcacd50d676))
* add can_upgrade() method to Caller and Sender
([50cf781](https://github.com/hoodie/hannibal/commit/50cf7813cbb2859097c65c7c4b942e273117e6e2))
* actors can be asked if they are stopped already
([5befdb8](https://github.com/hoodie/hannibal/commit/5befdb8661d5ed52d551297db1aa238d336211e5))
* add extra stop_supervisor() method to Addr and Context
([72230a3](https://github.com/hoodie/hannibal/commit/72230a37cfb1323a96c9808a06b063ac96b213a3))
* rename to hannibal
([d7dd843](https://github.com/hoodie/hannibal/commit/d7dd843802dd4d4f4e3d7e433daccf6194bf724e))

### [v0.7.11](https://github.com/hoodie/hannibal/compare/v0.7.10...v0.7.11) (2020-12-29)

### [v0.7.10](https://github.com/hoodie/hannibal/compare/v0.7.7...v0.7.10) (2020-12-29)

### [v0.7.7](https://github.com/hoodie/hannibal/compare/v0.7.5...v0.7.7) (2020-08-04)

### [v0.7.5](https://github.com/hoodie/hannibal/compare/v0.7.3...v0.7.5) (2020-07-03)

### [v0.7.3](https://github.com/hoodie/hannibal/compare/v0.7.2...v0.7.3) (2020-06-09)

### [v0.7.2](https://github.com/hoodie/hannibal/compare/v0.7.1...v0.7.2) (2020-06-08)

### [v0.7.1](https://github.com/hoodie/hannibal/compare/v0.7.0...v0.7.1) (2020-06-04)

## [v0.7.0](https://github.com/hoodie/hannibal/compare/v0.6.6...v0.7.0) (2020-06-03)

### [v0.6.6](https://github.com/hoodie/hannibal/compare/v0.6.5...v0.6.6) (2020-06-02)

### v0.6.5 (2020-06-02)
