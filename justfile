test:
    cargo test --no-default-features --features runtime-tokio,anyhow
    cargo test --no-default-features --features runtime-async-std,anyhow
    cargo test --no-default-features --features runtime-tokio,eyre
    cargo test --no-default-features --features runtime-async-std,eyre
    cargo test --no-default-features --features runtime-tokio,tracing
semver-checks:
    # cargo semver-checks --only-explicit-features --features default
    cargo semver-checks --only-explicit-features --features runtime-tokio --features anyhow
    cargo semver-checks --only-explicit-features --features runtime-async-std --features anyhow
    cargo semver-checks --only-explicit-features --features runtime-tokio --features eyre
    cargo semver-checks --only-explicit-features --features runtime-async-std --features eyre
