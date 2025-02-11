default:
  just --list

build:
  cargo build

check:
   cargo fmt
   cargo clippy --all-targets

test:
  cargo test --lib
  cargo test --doc

sync: 
  cargo test signet_syncs -- --nocapture

integrate: 
  cargo test -- --test-threads 1 --nocapture --skip signet_syncs

example:
  cargo run --example signet --release

signet:
  cargo run --example signet --release

testnet:
  cargo run --example testnet --release

all:
  cargo fmt 
  cargo clippy --all-targets
  cargo test --lib
  cargo test --doc
  cargo test -- --test-threads 1 --nocapture --skip signet_syncs
  cargo run --example signet
