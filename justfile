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


export NEXTEST_HIDE_PROGRESS_BAR:="true"

test $NEXTEST_STATUS_LEVEL="slow" $NEXTEST_FAILURE_OUTPUT="final" $NEXTEST_FINAL_STATUS_LEVEL="slow":
    cargo nextest run --workspace --no-fail-fast
    # cargo nextest run --workspace --no-fail-fast --all-targets
    # cargo nextest run --workspace --no-fail-fast --lib --no-default-features --features tokio_runtime
    # cargo nextest run --workspace --no-fail-fast --lib --no-default-features --features async_runtime
    # cargo nextest run --workspace --no-fail-fast --lib --no-default-features --features async_runtime,tokio

build-examples:
    cargo build --manifest-path hannibal-examples/Cargo.toml --features tokio_runtime

ci: clippy test build-examples
