list:
  @just --list

build:
  RUSTFLAGS="-C target-feature=+crt-static" cargo build --target x86_64-unknown-linux-gnu --release
  cp target/x86_64-unknown-linux-gnu/release/decatholac-mango-tarnished ./decatholac-mango-tarnished

build-windows:
  RUSTFLAGS="-C target-feature=+crt-static" cargo build --target x86_64-pc-windows-gnu --release
  cp target/x86_64-pc-windows-gnu/release/decatholac-mango-tarnished.exe ./decatholac-mango-tarnished.exe
