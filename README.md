# Kyoto Light Client

⚠️ **Warning**: This project is under development and is not suitable for actual use ⚠️

## Description

Kyoto is aiming to be a light-weight and private Bitcoin client. While [Neutrino](https://github.com/lightninglabs/neutrino/blob/master) is the standard SPV for [LND](https://github.com/lightningnetwork/lnd), integrations with existing Rust clients for [LDK](https://github.com/lightningdevkit) and [BDK](https://github.com/bitcoindevkit) haven't come to furition. The [Nakamoto](https://github.com/cloudhead/nakamoto) project is complete with some very modular, elegant programming, but the lead maintainer has other projects to focus on. [Murmel](https://github.com/rust-bitcoin/murmel) is yet another light client in Rust, but the last commit was 4 years ago at the time of writing. The Rust community of crates has evolved quickly in terms of asynchronus frameworks and runtime executors. Like the [LDK node](https://github.com/lightningdevkit/ldk-node?tab=readme-ov-file) project, this project leverages the use of `tokio`. By leveraging how futures and executors have developed over the years, the hope is a light client in Rust should be significantly easier to maintain. To read more about the scope, usage recommendations, and implementation details, see [DETAILS.md](./DETAILS.md).

## Running an example

To run the Signet example, fork the project and run: `cargo run --example signet` in the root directory.

#### Getting Started

The following snippet demonstrates how to build a Kyoto node. See the docs for more details on the `NodeBuilder`, `Node`, `Client`, and more.

```rust
use kyoto::node::NodeBuilder;
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
