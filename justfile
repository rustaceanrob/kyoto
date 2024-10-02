bitcoindir := "$HOME/.bitcoin/"

default:
  just --list

build:
  cargo build

check:
   cargo fmt
   cargo clippy

test:
  cargo test -- --skip test_signet_syncs

sync: 
  cargo test test_signet_syncs -- --nocapture

integrate: 
  sh scripts/integration.sh {{bitcoindir}}

example:
  cargo run --example signet

all:
  cargo fmt 
  cargo clippy --all-targets
  cargo test -- --skip test_signet_syncs
  sh scripts/integration.sh {{bitcoindir}}
  cargo run --example signet
