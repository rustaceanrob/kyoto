set ignore-comments
  
# Hidden default lists all available recipies.
_default:
  @just --list --list-heading $'KYOTO\n'
  
# Quick check of the code including lints and formatting.
check:
  cargo fmt -- --check
  # Turn warnings into errors.
  cargo clippy --all-targets -- -D warnings
  cargo check --all-features

# Run a test suite: unit, integration, features, msrv, or sync.
test suite="unit":
  just _test-{{suite}}

# Unit test suite.
_test-unit:
  cargo test --lib
  cargo test --doc
  cargo test --examples

# Run integration tests, excluding the network sync.
_test-integration: 
  cargo test --tests -- --test-threads 1 --nocapture --skip signet_syncs

# Run the network sync integration test.
_test-sync: 
  cargo test signet_syncs -- --nocapture

# Test feature flag matrix compatability.
_test-features:
  # Build and test with all features, no features, and some combinations.
  cargo test --lib --all-features
  cargo test --lib --no-default-features
  cargo test --lib --no-default-features --features rusqlite,filter-control 

# Check code with MSRV compiler.
_test-msrv:
  # Handles creating sandboxed environments to ensure no newer binaries sneak in.
  cargo install cargo-msrv@0.18.4
  cargo msrv verify --all-features

# Run the example: signet or testnet.
example name="signet":
  cargo run --example {{name}} --release

# Delete unused files or branches: data, lockfile, branches
delete item="data":
  just _delete-{{item}}

_delete-data:
  rm -rf light_client_data

_delete-lockfile:
  rm Cargo.lock

_delete-branches:
  git branch --merged | grep -v \* | xargs git branch -d
