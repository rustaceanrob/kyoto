# Kyoto Light Client

⚠️ **Warning**: This project is under development and is not suitable for actual use ⚠️

## Description

Kyoto is aiming to be a light-weight and private Bitcoin client. While [Neutrino](https://github.com/lightninglabs/neutrino/blob/master) is the standard slim client for [LND](https://github.com/lightningnetwork/lnd), integrations with existing Rust clients for [LDK](https://github.com/lightningdevkit) and [BDK](https://github.com/bitcoindevkit) haven't come to furition. The [Nakamoto](https://github.com/cloudhead/nakamoto) project is complete with some very modular, elegant programming, but the lead maintainer has other projects to focus on. [Murmel](https://github.com/rust-bitcoin/murmel) is yet another light client in Rust, but the last commit was 4 years ago at the time of writing. The Rust community of crates has evolved quickly in terms of asynchronus frameworks and runtime executors. Like the [LDK node](https://github.com/lightningdevkit/ldk-node?tab=readme-ov-file) project, this project leverages the use of `tokio`. By leveraging how futures and executors have developed over the years, the hope is a light client in Rust should be significantly easier to maintain.

## Running an example

To run the Signet example, fork the project and run: `cargo run --example signet` in the root directory.

#### Getting Started

The following snippet demonstrates how to build a Kyoto node. See the docs for more details on the `NodeBuilder`, `Node`, `Client`, and more.

```rust
use kyoto::node::NodeBuilder;
let builder = NodeBuilder::new(bitcoin::Network::Signet);
// Add node preferences and build the node/client
let (mut node, mut client) = builder
    // Add the peers
    .add_peers(vec![(peer, 38333), (peer_2, 38333)])
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
    .await;
```

## Scope

#### Functional Goals

- [x] Provide an archival index for blocks and transactions related to a set of `scriptPubKey`, presumably because the user is interested in transactions with these scripts involved.
- [x] Provide an interface to the P2P network, particularly to allow for new transaction broadcasting. Once BIP-324 is integrated, communication over the P2P network will also be encrypted.
- [x] Provide a testing ground for experimentation and research into the Bitcoin P2P network.
- [x] Provide rudimentary blockchain data, like the height of the chain, the "chainwork", the `CompactTarget` of the last block, etc.

#### Out of Scope

- Any wallet functionality beyond indexing transactions. This includes balances, transaction construction, etc. Bitcoin wallets are complex for a number of reasons, and additional functionality within this scope would detract from other improvements.

With these few simple goals in mind, the tools are set out for developers to create Bitcoin applications that directly interface with the Bitcoin protocol. The scope of such wallets is incredibly large, from a Lightning Network wallet running on a mobile device to a DLC client monitoring a bet. The privacy tradeoffs of using a light client like Kyoto far exceed that of using a chain oracle where the user inquires for transactions _directly_. With Kyoto, full network nodes only know that they have sent you an entire _block_, which can, and most likely will, contain thousands of transactions. If you would like to read more about the use cases for light clients, you can read my [blog post](https://robnetzke.com/blog/13-clients).