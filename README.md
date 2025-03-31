<div align="center">
  <h1>Kyoto: Bitcoin Light Client</h1>
  <p>
    <strong>An Implementation of BIP-157/BIP-158</strong>
  </p>

  <p>
    <a href="https://crates.io/crates/kyoto-cbf"><img alt="Crate Info" src="https://img.shields.io/crates/v/kyoto-cbf.svg"/></a>
    <a href="https://github.com/rustaceanrob/kyoto/blob/master/LICENSE"><img alt="MIT or Apache-2.0 Licensed" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg"/></a>
    <a href="https://github.com/rustaceanrob/kyoto/actions?query=workflow%3A%22Build+%26+Test%22"><img alt="CI Status" src="https://github.com/rustaceanrob/kyoto/workflows/Build%20%26%20Test/badge.svg"></a>
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
use std::str::FromStr;
use std::collections::HashSet;
use kyoto::{NodeBuilder, Log, Event, Client, Address, Network, HeaderCheckpoint, BlockHash};
#[tokio::main]
async fn main() {
    // Add third-party logging
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    // Add Bitcoin scripts to scan the blockchain for
    let address = Address::from_str("tb1q9pvjqz5u5sdgpatg3wn0ce438u5cyv85lly0pc")
        .unwrap()
        .require_network(Network::Signet)
        .unwrap()
        .into();
    let mut addresses = HashSet::new();
    addresses.insert(address);
    // Start the scan after a specified header
    let checkpoint = HeaderCheckpoint::closest_checkpoint_below_height(170_000, Network::Signet);
    // Create a new node builder
    let builder = NodeBuilder::new(Network::Signet);
    // Add node preferences and build the node/client
    let (mut node, client) = builder
        // The Bitcoin scripts to monitor
        .add_scripts(addresses)
        // Only scan blocks strictly after an anchor checkpoint
        .anchor_checkpoint(checkpoint)
        // The number of connections we would like to maintain
        .num_required_peers(2)
        .build_node()
        .unwrap();
    // Run the node and wait for the sync message;
    tokio::task::spawn(async move { node.run().await });
    // Split the client into components that send messages and listen to messages
    let Client { requester, mut log_rx, mut event_rx } = client;
    // Sync with the single script added
    loop {
        tokio::select! {
            log = log_rx.recv() => {
                if let Some(log) = log {
                    match log {
                        Log::Dialog(d) => tracing::info!("{d}"),
                        _ => (),
                    }
                }
            }
            event = event_rx.recv() => {
                if let Some(event) = event {
                    match event {
                        Event::Synced(_) => {
                            tracing::info!("Sync complete!");
                            break;
                        },
                        _ => (),
                    }
                }
            }
        }
    }
    requester.shutdown().await;
```

## Minimum Supported Rust Version (MSRV) Policy

The `kyoto` core library with default features supports an MSRV of Rust 1.63.

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

## Contributing

Please read [CONTRIBUTING.md](./CONTRIBUTING.md) to get started.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.
