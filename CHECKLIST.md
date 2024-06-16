## Development Checklist

#### Peers

- [x] Bootstrap peer list with DNS
  - [ ] Home brewed DNS resolver?
  - [ ] Check for DNS flooding/poisoning?
- [x] Persist to storage
  - [ ] Organize by `/16`?
  - [ ] Weight the priorities of high probability connections (DNS), service flags, and new peer discovery
  - [ ] Condense to single DB
- [ ] Ban peers
- [x] Add optional whitelist

#### Headers

- [x] Sync to known checkpoints with a designated "sync peer"
- [ ] Validation
  - [x] Median time past
  - [x] All headers connect
  - [x] No forks before last known checkpoint
  - [x] Header pass their own PoW
  - [ ] Difficulty retargeting audit:
    - [x] [PR](https://github.com/rust-bitcoin/rust-bitcoin/pull/2740)
  - [ ] Network adjusted time
- [x] Handle forks [took the Neutrino approach and just disconnect peers if they send forks with less work]
  - [ ] Manage orphaned header chains
  - [x] Extend valid forks
  - [ ] Create new forks
  - [x] Try to reorg when encountering new forks
  - [ ] Take the old best chain and make it a fork
- [x] Persist to storage
  - [x] Determine if the block hash or height should be the primary key
  - [x] Speed up writes with pointers
  - [x] Add "write volatile" to write over heights
- [x] Exponential backoff for locators

#### Filters

- [ ] API
  - [ ] Compute block filter from block?
  - [x] Check set inclusion given filter
- [ ] Chain
  - [x] Manage a queue of proposed header chains
  - [x] Find disputes
  - [x] Broadcast the next CF header message to all peers
  - [ ] Resolve disputes by downloading blocks?
  - [x] Add new filters to the chain, verifying with the `FilterHash`
- [ ] Optimizations
  - [x] Hashmap the `BlockHash` to `FilterHash` relationship in memory
  - [ ] Persist SPKs that have already been proven to be in a filter?

#### Main thread

- [x] Respond to peers with next `getheader` message
- [x] Manage the number of peers and disconnects
- [x] Organize the peers in a `BTreeMap` or similar
  - [x] Poll handles for progress
  - [x] Designate a "sync" peer
  - [x] Track "network adjusted time"
- [x] Have some `State` to manage what messages to send out
- [x] Seed with SPKs and wallet "birthday"
  - [x] Add SPKs
  - [x] Build from `HeaderCheckpoint`
- [x] Rescan with new `ScriptBuf`

#### Peer threads

- [x] Reach out with v1 version message
- [x] Respond to `Ping`
- [x] Send `Verack` and eagerly send `GetAddr`
  - [ ] May limit addresses if peer persistence is saturated
- [x] Filter messages at the reader level
  - [ ] Add back: `Inv`, `Block`, `TX`, ?
    - [x] `Inv` (blocks)
    - [x] `Block`
  - [x] Update `Inv` of block headers to header chain
- [ ] Set up "peer config"
  - [x] TCP timeout
  - [ ] Should ask for IP addresses
  - [x] Should serve CPF
- [ ] Set up "timer"
  - [x] Check for DOS
  - [x] Message counter
  - [ ] `Ping` if peer has not been heard from
- [ ] `Disconnect` peers with high latency
- [ ] Add BIP-324 with V1 fallback

#### Transaction Broadcaster

- [ ] Rebroadcast for every TX not included in new blocks (`Inv`)
- [x] Add `ScriptBuf` to script set

#### Meta

- [x] Add more error cases for loading faulty headers from persistence
- [x] Handle `Inv` during CF header download
- [ ] Add local unconfirmed transaction DB?
- [ ] Too many `clone`

#### Testing

- [ ] Chain
  - [x] Usual extend
  - [x] Fork with less work
  - [ ] Orphaned fork
  - [x] Fork with equal work
  - [x] Fork with more work
- [ ] CF header chain
  - [ ] Unexpected stop hash
  - [ ] Unexpected filter hash
  - [ ] Multiple peers expected filter hash
  - [ ] Properly identify bad peers
- [ ] Filter chain
  - [ ] Repeated filter
  - [ ] Bad filter
- [ ] Header Chain
  - [x] Expected height
  - [x] Expected height after fork
  - [x] Expected hash at height
  - [x] Expected work after height
  - [x] Properly handles fork
  - [x] `extend` handles forks, including down to the anchor
- [ ] CI
  - [x] MacOS, Windows, Linux
  - [x] 1.63, stable, beta, nightly
  - [x] Format and clippy
  - [ ] Regtest sync with Bitcoin Core
  - [ ] On PR

#### Bindings

- [ ] Add UniFFI to repository
- [ ] Build UDL
- [ ] Build for Python
