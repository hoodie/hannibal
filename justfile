test:
    cargo test --no-default-features --features runtime-tokio
    cargo test --no-default-features --features runtime-async-std
    cargo test --no-default-features --features runtime-tokio,tracing
semver-checks:
    # cargo semver-checks --only-explicit-features --features default
    cargo semver-checks --only-explicit-features --features runtime-tokio
    cargo semver-checks --only-explicit-features --features runtime-async-std
