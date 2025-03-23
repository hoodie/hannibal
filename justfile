default:
    just --list

coverage:
    # cargo llvm-cov test --lib --no-default-features --features async-std
    # cargo llvm-cov test --lib --no-default-features --features tokio
    # cargo llvm-cov test --lib
    cargo llvm-cov --html test --lib
    open target/llvm-cov/html/index.html

clippy:
    cargo --quiet clippy --workspace --quiet
    cargo --quiet clippy --workspace --quiet --lib --tests --no-default-features --features tokio
    cargo --quiet clippy --workspace --quiet --lib --tests --no-default-features --features async-std


test $NEXTEST_STATUS_LEVEL="slow":
    cargo nextest run --workspace
    cargo nextest run --workspace --all-targets --no-default-features
    cargo nextest run --workspace --lib --no-default-features --features tokio
    cargo nextest run --workspace --lib --no-default-features --features async-std

ci: clippy test
