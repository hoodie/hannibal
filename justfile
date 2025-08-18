default:
    just --list

coverage:
    # cargo llvm-cov test --lib --no-default-features --features async_runtime
    # cargo llvm-cov test --lib --no-default-features --features tokio_runtime
    # cargo llvm-cov test --lib
    cargo llvm-cov --html test --lib
    open target/llvm-cov/html/index.html

clippy:
    cargo --quiet clippy --workspace --quiet
    cargo --quiet clippy --workspace --quiet --lib --tests --no-default-features --features tokio_runtime
    cargo --quiet clippy --workspace --quiet --lib --tests --no-default-features --features async_runtime
    cargo --quiet clippy --workspace --quiet --lib --tests --no-default-features --features async_runtime,tokio


test $NEXTEST_STATUS_LEVEL="slow":
    cargo nextest run --workspace
    cargo nextest run --workspace --all-targets
    cargo nextest run --workspace --lib --no-default-features --features tokio_runtime
    cargo nextest run --workspace --lib --no-default-features --features async_runtime
    cargo nextest run --workspace --lib --no-default-features --features async_runtime,tokio

build-examples:
    cargo build --manifest-path hannibal-examples/Cargo.toml --features tokio_runtime

ci: clippy test build-examples
