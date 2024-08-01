---
theme:
  name: terminal-light
---

# Road to compact filters

- `rust-bitcoin` provides the robust types to parse P2P messages
- `bdk_chain` has nice methods on `IndexedTxGraph` like `apply_block_relevant`
- Missing link is a library to find and maintain TCP connections, interpret P2P messages, and relay relevant blocks to BDK
- My project `https://github.com/rustaceanrob/kyoto` attempts to fill the gaps

<!-- end_slide -->

# Miscellaneous Details

- Configurable with known peers or may be completely bootstrapped with DNS
- Supports arbitrary number of outbound connections (although finding CBF peers can be challenging)
- Scripts may be added up front when building or as the node is running
- Calling `Node::run` will run the node continuously while the application is running
  - Usable on the "client side" or an always-on "server side" application
- `rust-bitcoin` handles serialization and deserialization of P2P messages, and `tokio` enables flexible concurrency

```toml
[dependencies]
bitcoin_hashes = "0.14.0"
bitcoin = { version = "0.32.0", features = [
    "serde",
    "rand-std",
], default-features = false }
tokio = { version = "1.37", default-features = false, features = [
    "rt-multi-thread",
    "sync",
    "time",
    "io-util",
    "net",
    "macros",
] }

# Optional dependencies
rusqlite = { version = "0.31.0", features = ["bundled"], optional = true }
```

<!-- end_slide -->

# Swift example

<!-- end_slide -->
