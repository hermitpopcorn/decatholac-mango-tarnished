# Builds for production
RUSTFLAGS="-C target-feature=+crt-static" cargo build --target x86_64-pc-windows-gnu --release
