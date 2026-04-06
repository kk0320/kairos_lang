cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release --workspace
Write-Host "Ready to tag and release Kairos v2.0.0."
