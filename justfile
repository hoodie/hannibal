default:
    just --list

coverage:
    # cargo llvm-cov test --lib --no-default-features --features async-std
    # cargo llvm-cov test --lib --no-default-features --features tokio
    # cargo llvm-cov test --lib
    cargo llvm-cov --html test --lib
    open target/llvm-cov/html/index.html

clippy:
    cargo clippy
    cargo clippy --lib --tests --no-default-features --features tokio
    cargo clippy --lib --tests --no-default-features --features async-std


test:
    cargo --quiet test
    cargo --quiet test --tests --no-default-features
    cargo --quiet test --lib --no-default-features --features tokio
    cargo --quiet test --lib --no-default-features --features async-std

ci: test
