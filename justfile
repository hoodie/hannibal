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


test $RUST_LOG="trace" $NEXTEST_STATUS_LEVEL="slow" $NEXTEST_FAILURE_OUTPUT="final" $NEXTEST_FINAL_STATUS_LEVEL="slow" $STRESS_COUNT="3":
    cargo nextest run --workspace --all-targets --stress-count $STRESS_COUNT
    cargo nextest run --workspace --lib --no-default-features --features tokio_runtime --stress-count $STRESS_COUNT
    cargo nextest run --workspace --lib --no-default-features --features async_runtime --stress-count $STRESS_COUNT
    cargo nextest run --workspace --lib --no-default-features --features async_runtime,tokio --stress-count $STRESS_COUNT

install-deps:
    cargo install cargo-nextest
    cargo install cargo-semver-checks

semver-checks:
    cargo semver-checks

build-examples:
    cargo build --manifest-path hannibal-examples/Cargo.toml --features tokio_runtime

ci: install-deps clippy test build-examples semver-checks
