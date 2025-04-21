# Changelog

Notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.10.0

## Added

- Template issue for bugs, enhancement requests, releases
- Mainnet, signet, testnet4 checkpoints
- Testnet4 DNS seeds
- Socks5 proxy connections supported
- `justfile` improvements

## Changed

- Removed the crate lockfile
- Removed the `core` module
- Removed the `filter` module
- Renamed `DisconnectedHeader` to `IndexedHeader`
- `database` feature is now `rusqlite`
- Remove `arti` and Tor feature in favor of a Socks5 proxy
- Folder for data storage is now `light_client_data`
- SQL header schema has been changed to store `BLOB` of consensus encoded header bytes
- New log level introduced and `Log` has been renamed to `Info`
- Debug strings sent on a separate channel
- Sending messages to the node is synchronous
- Checking `IndexedFilter` for matches is immutable

## Fixes
- Introduce `BlockTree` to manage chain data
    - Constant time data access when indexed by height or hash
    - Consolidated block headers, filter headers/hashes, filter checks into a single struct
    - Management of candidate forks
    - Constant time fork comparison
- CI refactors and improvements

## 0.9.0

## Added

- Introduce log level and optimize release builds to remove heap allocations for debug messages
- Configure a custom DNS resolver

## Changed

- `Dialog` field renamed to `Debug`
- `dns` feature is removed and DNS is used by default
- Better naming on the fields of `Warning`
- `NodeBuilder` uses declarative naming for methods

## Fixes

- Tor and Signet examples updated
- Adding scripts or peers twice does not overwrite past changes in `NodeBuilder`
- Remove invalid assessment of median time past in fork scenario
- Use the proper `inv` -> `getdata` -> `tx` message exchange to broadcast transactions

## 0.8.0

## Added

- Request the broadcast minimum fee rate from connected peers.

## Changed

- Removed the `StatelessPeerStore`, used primarily for development
- Export the `tokio` crate
- Further split `Log` and `Event` enumerations into `Log`, `Warning`, and `Event`

## Fixes

- Update the port and services when peers are gossiped
- Reset the timer after disconnecting from peers due to a stale block
- Remove case for invalid median time check

## 0.7.0

## Added

- Request a block using the `Client`
- Add `broadcast_random` convenience method on `Client`
- Request a `Range` of block headers from `Client`

## Changed

- Separate logs from events into different event channels
    - The `Log` channel is bounded in size and contains informational, but non-critical information
    - The `Event` channel is unbounded and contains data that must be handled, like `IndexedBlock`
- Switch to `corepc-node` instead of unmaintained `bitcoincore-rpc`
- Load block headers with `RangeBounds`

## Fixes

- Remove unnecessary `unwrap` when managing filter headers
- Clamp connections to a defined range

## v0.6.0

## Added

- Pass `FeeFilter` to client
- Add Signet and Bitcoin checkpoints

## Changed

- Upgrade `bip324` to `0.6.0`
- Switch to `corepc-node` to start `bitcoind` in CI
- Use `into_payload` in `bitcoin 0.32.5`

## Fixes

- Add check to bits before adjustment
- Remove explicit `serde` feature

## v0.5.0

## Added

- Client may fetch a `Header` at a particular height
- Support for Testnet4 with new example

## Changed

- `HeaderStore` has additional `header_at` method
- Removed unused `IndexedTransaction` variant on `NodeMessage`
- New `Progress` variant on `NodeMessage`

## Fixes

- Use inline docs for rustdoc
- Check the `CompactTarget` of the block headers received with far stricter requirements with respect to the difficulty adjustment
- Bump `bip324` to `0.5.0` and `bitcoin` to `0.32.4`

## v0.4.0

## Added

- New `HeaderCheckpoint` constructor from height
- `shutdown`, `add_scripts`, `broadcast_transaction` methods have blocking APIs
- Add a `TrustedPeer` while the node is running
- Add change the peer timeout while the node is running

## Changed

- Use `impl Into` whenever possible on `NodeBuilder` and `Client` APIs
- Remove the misleading `wait_for_broadcast` method on `Client`

## Fixes

- Remove `Option` from `Whitelist` as it is already a `Vec`
- Limit the amount of `ADDR` messages a single peer can send

## v0.3.0

## Added

- Type alias for `Node` in `builder` with default generics

## Changed

- `HeaderStore` and `PeerStore` traits now have an associated error type
- `Node` is now generic over `H: HeaderStore` and `P: PeerStore`
- Move `NodeError` subvariants into `core`

## v0.2.0

## Added

- `just` and `justfile` for local development
- Additional SQL tests with `tempfile`
- Additional context to database read and write errors
- Support for a new silent payments feature-flag:
  - Receive block filters directly
  - Request blocks directly
  - Pause the node state before downloading filters

## Changed

- Disconnect peers if no block is found after 30 minutes
- Find new peers after 3 hour connections
- `Node::run` is an immutable method
- Check block merkle root
- Download blocks in parallel with filters
- Single `ScriptBuf` may be added at a time
- `Client` and `ClientSender` share methods

## Fixes

- Only request headers for `inv` the node does not have
- Max v2 handshake buffer size increased
- Changes to `BlockQueue` to not spam peers with requests
- Internal module renames

## v0.1.0

## Added

- `NodeBuilder` offers configuration options to build a compact block filters `Node` and `Client`
- `Client` is further split into `ClientSender` and `Receiver<NodeMessage>`
- `ClientSender` may: add scripts, broadcast transactions, rescan block filters, stop the node
- `Node` may run and send `NodeMessage`, `Warning` while running
- Connections to peers are encrypted if `ServiceFlags::P2P_V2` is signaled
- Connections are terminated if peers do not respond within a custom `Duration`
- Peers are selected at random with a target preference of 50% new and 50% tried
- Connections are terminated after long duration times to find new reliable peers
- Blocks are considered "stale" if there has not been a new inv message after 30 minutes. Block headers are requested again to ensure no inv were missed.
- Transactions are broadcast according to a configurable policy, either to all connections for higher reliability, or to a random peer for better privacy
- Default implementers of `PeerStore` and `HeaderStore` are `SqlitePeerDb` and `SqliteHeaderDb`
- Nodes will no peers will bootstrap with DNS
- Experimental support for connections routed over Tor
