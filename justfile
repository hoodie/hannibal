default:
    just --list

coverage:
    cargo llvm-cov --html test
    open target/llvm-cov/html/index.html
