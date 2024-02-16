```bash
cargo +stage2 build --target=x86_64-unknown-hyperion --bin=wm --bin=term
cp $CARGO_HOME/target/x86_64-unknown-hyperion/debug/{wm, term} ./asset/bin/
cargo run -- --cpus=1
```
