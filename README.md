# Kyoto Light Client

⚠️ **Warning**: This project is under development and is not suitable for actual use ⚠️

## Description

Kyoto is aiming to be a light-weight and private Bitcoin client. While [Neutrino](https://github.com/lightninglabs/neutrino/blob/master) is the standard slim client for [LND](https://github.com/lightningnetwork/lnd), integrations with existing Rust clients for [LDK](https://github.com/lightningdevkit) and [BDK](https://github.com/bitcoindevkit) haven't come to furition. The [Nakamoto](https://github.com/cloudhead/nakamoto) project is complete with some very modular, elegant programming, but the lead maintainer has other projects to focus on. [Murmel](https://github.com/rust-bitcoin/murmel) is yet another light client in Rust, but the last commit was 4 years ago at the time of writing. The Rust community of crates has evolved quickly in terms of asynchronus frameworks and runtime executors. Like the [LDK node](https://github.com/lightningdevkit/ldk-node?tab=readme-ov-file) project, this project leverages the use of `tokio` with plans to integrate UniFFI in the future. By leveraging how these frameworks have developed over the years, the hope is a light client in Rust should be significantly easier to maintain.

## Running an example

The folder `example` contains programs that use Kyoto find relevant blocks and transactions for a set of scripts. To run the Signet example, use: `cargo run --example signet`.

## Scope

#### Functional Goals

- [x] Provide rudimentary blockchain data, like the height of the chain, the "chainwork", the `CompactTarget` of the last block, etc.
- [x] Provide an archival index for transactions related to a set of `scriptPubKey`, presumably because the user is interested in transactions with these scripts involved.
- [x] Provide an interface to the P2P network, particularly to allow for new transaction broadcasting. Once BIP-324 is integrated, access to the P2P will also be encrypted.
- [x] Provide a testing ground for experimentation and research into the Bitcoin P2P network.

With these few simple goals in mind, the tools are set out for developers to create Bitcoin applications that directly interface with the Bitcoin protocol. The scope of such wallets is incredibly large, from a Lightning Network wallet running on a mobile device to a federated mint where some members do not always index the full blockchain. The privacy tradeoffs of using a light client like Kyoto far exceed that of using a chain oracle where the user inquires for transactions _directly_. With Kyoto, full network nodes only know that they have sent you an entire _block_, which can, and most likely will, contain thousands of transactions. If you would like to read more about the use cases for light clients, you can read my [blog post](https://robnetzke.com/blog/13-clients).

#### Out of Scope

- Any wallet functionality beyond indexing transactions. This includes balances, transaction construction, etc. Why? Bitcoin wallets are complex for a number of reasons, and additional functionality within this scope would detract from other improvements.

## Checklist

#### Peers

- [x] Bootstrap peer list with DNS
  - [ ] Home brewed DNS resolver?
  - [ ] Check for DNS flooding/poisoning?
- [x] Persist to storage
  - [ ] Organize by `/16`?
  - [ ] Weight the priorities of high probability connections (DNS), service flags, and new peer discovery
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
  - [ ] Add "write volatile" to write over heights
- [x] Exponential backoff for locators

#### Filters

- [ ] API
  - [ ] Compute block filter from block
  - [x] Check set inclusion given filter
- [ ] Chain
  - [x] Manage a queue of proposed header chains
  - [x] Find disputes
  - [x] Broadcast the next CF header message to all peers
  - [ ] Resolve disputes by downloading blocks
  - [x] Add new filters to the chain, verifying with the `FilterHash`
- [ ] Optimizations
  - [x] Hashmap the `BlockHash` to `FilterHash` relationship in memory
  - [ ] Persist SPKs that have already been proven to be in a filter

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
- [ ] Rescan with new `ScriptBuf`

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
    - [ ] Filter by CPF
  - [ ] Should serve CPF
- [ ] Set up "timer"
  - [x] Check for DOS
  - [ ] `Ping` if peer has not been heard from
- [ ] `Disconnect` peers with high latency
- [ ] Add BIP-324 with V1 fallback

#### Meta

- [x] Add more error cases for loading faulty headers from persistence
- [ ] Add local unconfirmed transaction DB
- [ ] Add archival transaction DB
- [ ] Too many `clone`

#### Testing

- [ ] Header chain
  - [ ] Usual extend
  - [ ] Fork with less work
  - [ ] Orphaned fork
  - [ ] Fork with equal work
  - [ ] Fork with more work
- [ ] CF header chain
  - [ ] Unexpected stop hash
  - [ ] Unexpected filter hash
  - [ ] Multiple peers expected filter hash
  - [ ] Properly identify bad peers
- [ ] Filter chain
  - [ ] Repeated filter
  - [ ] Bad filter
- [ ] Chain
  - [ ] Expected height
  - [ ] Expected height after fork
  - [ ] Expected hash at height
  - [ ] Properly handles fork
  - [ ] Incorrect filter hash at block
- [ ] CI

#### Bindings

- [ ] Add UniFFI to repository
- [ ] Build UDL
- [ ] Build for Python
