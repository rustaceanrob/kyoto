<div align="center">
  <h1>Kyoto: Bitcoin Light Client</h1>
  <p>
    <strong>An Implementation of BIP-157/BIP-158</strong>
  </p>

  <p>
    <a href="https://github.com/bitcoindevkit/bdk/blob/master/LICENSE"><img alt="MIT or Apache-2.0 Licensed" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg"/></a>
    <a href="https://github.com/rustaceanrob/kyoto/actions?query=workflow%3ACI"><img alt="CI Status" src="https://github.com/bitcoindevkit/bdk/workflows/CI/badge.svg"></a>
    <!-- <a href="https://docs.rs/bdk_wallet"><img alt="API Docs" src="https://img.shields.io/badge/docs.rs-bdk_wallet-green"/></a> -->
    <a href="https://blog.rust-lang.org/2022/08/11/Rust-1.63.0.html"><img alt="Rustc Version 1.63.0+" src="https://img.shields.io/badge/rustc-1.63.0%2B-lightgrey.svg"/></a>
  </p>
</div>

## About

⚠️ **Warning**: This project is under development and is not currently suitable for use ⚠️

Kyoto is aiming to be a simple, memory-conservative, and private Bitcoin client for developers to build wallet applications. To read more about the scope, usage recommendations, and implementation details, see [DETAILS.md](./doc/DETAILS.md).

## Running an example

To run the Signet example, in the root directory:

```
cargo run --example signet
```

#### Getting Started

The following snippet demonstrates how to build a Kyoto node. See the docs for more details on the `NodeBuilder`, `Node`, `Client`, and more.

```rust
use kyoto::NodeBuilder;
use kyoto::TrustedPeer;

let builder = NodeBuilder::new(bitcoin::Network::Signet);
// Add node preferences and build the node/client
let (mut node, mut client) = builder
    // Add the peers
    .add_peers(vec![TrustedPeer::from_ip(peer_1), TrustedPeer::from_ip(peer_1)])
    // The Bitcoin scripts to monitor
    .add_scripts(addresses)
    // Only scan blocks strictly after an anchor checkpoint
    .anchor_checkpoint(HeaderCheckpoint::new(
        180_000,
        BlockHash::from_str("0000000870f15246ba23c16e370a7ffb1fc8a3dcf8cb4492882ed4b0e3d4cd26")
            .unwrap(),
    ))
    // The number of connections we would like to maintain
    .num_required_peers(2)
    // Create the node and client
    .build_node()
```

#### Minimum Supported Rust Version (MSRV) Policy

The `kyoto` core library with default features supports an MSRV of Rust 1.63. To build the library with Rust 1.63, the `database` feature requires a pinned dependency: `cargo update -p allocator-api2 --precise "0.2.9"`. 

While connections over the Tor protocol are supported by the feature `tor`, the dependencies required cannot support the MSRV. As such, no MSRV guarantees will be made when using Tor, and the feature should be considered experimental.

#### Integration Testing

To run the integrations against your `bitcoind` instance, in the root of the project:

```
chmod +x scripts/integration.sh
sh scripts/integration.sh "path/to/bitcoin/folder"
```

To run the unit tests, `cargo fmt`, `clippy`, and an example:

```
sh scripts/pr.sh
```

