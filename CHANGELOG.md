# Changelog

Notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.0

#### Added

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
