# Kyoto Light Client

## Description

Kyoto is aiming to be a light-weight and private Bitcoin client. While [Neutrino](https://github.com/lightninglabs/neutrino/blob/master) is the standard slim client for [LND](https://github.com/lightningnetwork/lnd), integrations with existing Rust clients for [LDK](https://github.com/lightningdevkit) and [BDK](https://github.com/bitcoindevkit) haven't come to furition. The [Nakamoto](https://github.com/cloudhead/nakamoto) project is complete with some very modular, elegant programming, but the lead maintainer has other projects to focus on. [Murmel](https://github.com/rust-bitcoin/murmel) is yet another light client in Rust, but the last commit was 4 years ago at the time of writing. The Rust community of crates has evolved quickly in terms of asynchronus frameworks and runtime executors. Like the [LDK node](https://github.com/lightningdevkit/ldk-node?tab=readme-ov-file) project, this project leverages the use of the `tokio` runtime with plans to integrate UniFFI in the future. By leveraging how these frameworks have developed over the years, the hope is a light client in Rust should be significantly easier to maintain. The greatest advantage when in comes to getting light clients on mobile is the ability to use UniFFI bindings to build native code in Swift and Kotlin. Once the client is functional, there will be a great focus on lower resource devices like smart phones.

## Checklist

#### Peers
- [x] Bootstrap peer list with DNS
    - [ ] Home brewed DNS resolver?
    - [ ] Check for DNS flooding/poisoning?
- [x] Persist to storage 
    - [ ] Organize by `/16`?
    - [ ] Weight the priorities of high probability connections (DNS), service flags, and new peer discovery

#### Headers
- [x] Sync to known checkpoints with a designated "sync peer"
- [ ] Validation
    - [x] Median time past
    - [x] All headers connect
    - [x] No forks before last known checkpoint
    - [x] Header pass their own PoW
    - [ ] Difficulty retargeting audit: [PR](https://github.com/rust-bitcoin/rust-bitcoin/pull/2740)
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
    - [ ] Add "write volatile" to write over heights
- [ ] Test fork scenarios with Regtest

#### Filters
- [ ] API
    - [ ] Compute block filter from block
    - [ ] Check set inclusion given filter
- [ ] Chain
    - [ ] Figure out structure for filter header chain with multiple peers

#### Main thread
- [x] Respond to peers with next `getheader` message
- [x] Manage the number of peers and disconnects
- [x] Organize the peers in a `BTreeMap` or similar
    - [x] Poll handles for progress
    - [ ] Designate a "sync" peer
    - [x] Track "network adjusted time"
- [x] Have some `State` to manage what messages to send out
- [ ] Seed with SPKs and wallet "birthday"

#### Peer threads
- [x] Reach out with v1 version message
- [x] Respond to `Ping`
- [x] Send `Verack` and eagerly send `GetAddr`
    - [ ] May limit addresses if peer persistence is saturated
- [x] Filter messages at the reader level
    - [ ] Add back: `Inv`, `Block`, `TX`, ?   
    - [ ] Update `Inv` of block headers to header chain
- [ ] Set up "peer config"
    - [x] TCP timeout
    - [ ] Should ask for addresses
        - [ ] Filter by CPF
    - [ ] Should serve CPF
- [ ] Set up "timer"
    - [x] Check for DOS
    - [ ] `Ping` if peer has not been heard from
- [ ] `Disconnect` peers with high latency
- [ ] Add BIP-324 with V1 fallback

#### Meta
- [x] Add more error cases for loading faulty headers from persistence

#### Bindings
- [ ] Add UniFFI to repository
- [ ] Build UDL
- [ ] Build for Python