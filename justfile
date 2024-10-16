default:
    just --list

coverage:
    cargo llvm-cov --html test
    open target/llvm-cov/html/index.html

ci:
    cargo test --no-default-features --features tokio
    cargo test --no-default-features --features async-std
    just coverage --all-features --lib
