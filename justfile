default:
    echo 'Hello, world!'

coverage:
    cargo llvm-cov --html test
    open target/llvm-cov/html/index.html
