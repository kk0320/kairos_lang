cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
Write-Host "Ready to tag and release Kairos."
