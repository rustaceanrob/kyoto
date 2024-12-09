<div align="center">
  <h1>Kyoto: Bitcoin Light Client</h1>
  <p>
    <strong>An Implementation of BIP-157/BIP-158</strong>
  </p>

  <p>
    <a href="https://crates.io/crates/kyoto-cbf"><img alt="Crate Info" src="https://img.shields.io/crates/v/kyoto-cbf.svg"/></a>
    <a href="https://github.com/bitcoindevkit/bdk/blob/master/LICENSE"><img alt="MIT or Apache-2.0 Licensed" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg"/></a>
    <a href="https://github.com/rustaceanrob/kyoto/actions?query=workflow%3ACI"><img alt="CI Status" src="https://github.com/bitcoindevkit/bdk/workflows/CI/badge.svg"></a>
    <a href="https://docs.rs/kyoto-cbf"><img alt="API Docs" src="https://img.shields.io/badge/docs.rs-kyoto_cbf-green"/></a>
    <a href="https://blog.rust-lang.org/2022/08/11/Rust-1.63.0.html"><img alt="Rustc Version 1.63.0+" src="https://img.shields.io/badge/rustc-1.63.0%2B-lightgrey.svg"/></a>
  </p>
</div>

## About

Kyoto is aiming to be a simple, memory-conservative, and private Bitcoin client for developers to build wallet applications. To read more about the scope, usage recommendations, and implementation details, see [DETAILS.md](./doc/DETAILS.md).

## Running an example

To run the Signet example, in the root directory:

```
cargo run --example signet
```

Or, with `just`:

```
just example
```

## Getting Started

The following snippet demonstrates how to build a Kyoto node. See the [docs](https://docs.rs/kyoto-cbf) for more details on the `NodeBuilder`, `Node`, `Client`, and more.

```rust
use std::collections::HashSet;
use kyoto::{NodeBuilder, NodeMessage, Address, Network, HeaderCheckpoint, BlockHash, TrustedPeer};

let address = Address::from_str("tb1q9pvjqz5u5sdgpatg3wn0ce438u5cyv85lly0pc")
    .unwrap()
    .require_network(Network::Signet)
    .unwrap()
    .into();
let mut addresses = HashSet::new();
addresses.insert(address);
let builder = NodeBuilder::new(bitcoin::Network::Signet);
// Add node preferences and build the node/client
let (node, client) = builder
    // Add the peers
    .add_peers(vec![TrustedPeer::from_ip(peer_1), TrustedPeer::from_ip(peer_1)])
    // The Bitcoin scripts to monitor
    .add_scripts(addresses)
    // Only scan blocks after a block height
    .filter_startpoint(180_000)
    // The number of connections we would like to maintain
    .num_required_peers(2)
    // Create the node and client
    .build_node()
    .unwrap();
```

## Minimum Supported Rust Version (MSRV) Policy

The `kyoto` core library with default features supports an MSRV of Rust 1.63. To build the library with Rust 1.63, the `database` feature requires a pinned dependency: `cargo update -p allocator-api2 --precise "0.2.9"`.

While connections over the Tor protocol are supported by the feature `tor`, the dependencies required cannot support the MSRV. As such, no MSRV guarantees will be made when using Tor, and the feature should be considered experimental.

## Integration Testing

The preferred workflow is by using `just`. If you do not have `just` installed, check out the [installation page](https://just.systems/man/en/chapter_4.html).

To run the unit tests and doc tests:

```
just test
```

To sync with a live Signet node:

```
just sync
```

And to run scenarios against your `bitcoind` instance, set a `BITCOIND_EXE` environment variable to the path to `bitcoind`:

```
export BITCOIND_EXE = "/usr/path/to/bitcoind/"
```

You may want to add this to your bash or zsh profile.

To run the `bitcoind` tests:

```
just integrate
```

If you do not have `bitcoind` installed, you may simply run `just integrate` and it will be installed for you in the `build` folder.

## Project Layout

`chain`: Contains all logic for syncing block headers, filter headers, filters, parsing blocks. Also contains preset checkpoints for Signet, Regtest, and Bitcoin networks. Notable files: `chain.rs`

`core`: Organizes the primary user-facing components of the API. This includes both the `Node` and the `Client` that all developers will interact with, as well as the `NodeBuilder`. Importantly includes `peer_map.rs`, which is the primary file that handles peer threads by sending messages, persisting new peers, banning peers, and managing peer task handles. `node.rs` is the main application loop, responsible for driving the node actions. Notable files: `node.rs`, `peer_map.rs`, `builder.rs`, `client.rs`

`db`: Defines how data must be persisted with `traits.rs`, and includes some opinionated defaults for database components.

`filters`: Additional structures for managing compact filter headers and filters, used by `chain.rs`

`network`: Opens and closes connections, handles encryption and decryption of messages, generates messages, parses messages, times message response times, performs DNS lookups. Notable files: `peer.rs`, `reader.rs`, `parsers.rs`, `outbound_messages.rs`

## Contributing

Please read [CONTRIBUTING.md](./CONTRIBUTING.md) to get started.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.