## Development Checklist

#### Peers

- [x] Bootstrap peer list with DNS
  - [x] Home brewed DNS client
  - [x] Check for DNS flooding/poisoning? (Kind of: just limit to 256 peers per DNS query)
- [x] Persist to storage
  - [x] Organize by `/16`? (Just don't select peers from the same net group)
  - [x] Weight the priorities of high probability connections (DNS), service flags, and new peer discovery (Completed with random preference for new/tried)
  - [x] Condense to single DB
- [x] Ban peers
- [x] Add optional whitelist
- [x] Add in-memory `PeerStore` implementor

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
- [x] Handle forks (took the Neutrino approach and just disconnect peers if they send forks with less work)
  - [ ] Manage orphaned header chains (Not necessary if we just follow the chain of most work and store the headers)
  - [x] Extend valid forks
  - [x] Create new forks
  - [x] Try to reorg when encountering new forks
  - [ ] Take the old best chain and make it a fork (Again, we just drop it and assume we will pick it back up if it gets more work)
- [x] Persist to storage
  - [x] Determine if the block hash or height should be the primary key
  - [x] Speed up writes with pointers
  - [x] Add method to write over heights in reorg
  - [x] Move headers to DB when the `BTreeMap` is large
- [x] Exponential backoff for locators

#### Filters

- [ ] API
  - [ ] Compute block filter from block? (Non-trivial. The computed filter relies on previous outputs not contained in the block. We could estimate the `ScriptBuf` using the `Witness`?)
  - [x] Check set inclusion given filter
- [ ] Chain
  - [x] Manage a queue of proposed header chains
  - [x] Find disputes
  - [x] Broadcast the next CF header message to all peers
  - [ ] Resolve disputes by downloading blocks? (Again, hard to resolve the dispute if you can't compute the filter)
  - [x] Add new filters to the chain, verifying with the `FilterHash`
- [ ] Optimizations
  - [x] Hashmap the `BlockHash` to `FilterHash` relationship in memory
  - [ ] Persist SPKs that have already been proven to be in a filter? (Not necessary, the crate should remain mostly state-less)

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
  - [x] May limit addresses if peer persistence is saturated (Set desired size)
- [x] Filter messages at the reader level
  - [ ] Add back: `Inv`, `Block`, `TX`, ?
    - [x] `Inv` (blocks)
    - [x] `Block`
    - [ ] `Tx` (Open question/configuration)
  - [x] Update `Inv` of block headers to header chain
- [ ] Set up "peer config"
  - [x] TCP timeout
  - [x] Should ask for IP addresses (More of a DB level thing. We need peers if we are below a certain threshold)
  - [x] Should serve CPF
- [ ] Set up "timer"
  - [x] Check for DOS
  - [x] Message counter
  - [ ] `Ping` if peer has not been heard from (Probably better to just disconnect)
- [x] `Disconnect` peers with high latency (If we send a critical message and a peer doesn't respond in 5 seconds, disconnect)
- [x] Add BIP-324 with V1 fallback

#### Transaction Broadcaster

- [ ] Rebroadcast for every TX not included in new blocks (Not possible without persistence, probably unnecessary)
- [x] Add `ScriptBuf` to script set

#### Meta

- [x] Add more error cases for loading faulty headers from persistence
- [x] Handle `Inv` during CF header download
- [ ] Add local unconfirmed transaction DB? (Advised against by downstream)
- [x] Too many `clone`

#### Testing

- [ ] Chain
  - [x] Usual extend
  - [x] Fork with less work
  - [x] Orphaned fork
  - [x] Fork with equal work
  - [x] Fork with more work
- [ ] CF header chain
  - [x] Unexpected stop hash
  - [x] Unexpected filter hash
  - [x] Multiple peers expected filter hash
  - [ ] Properly identify bad peers (Not testable without a way to compute the filters)
- [ ] Filter chain
  - [x] Repeated filter
  - [x] Bad filter
- [ ] Header Chain
  - [x] Expected height
  - [x] Expected height after fork
  - [x] Expected hash at height
  - [x] Expected work after height
  - [x] Properly handles fork
  - [x] `extend` handles forks, including down to the anchor
- [ ] Bitcoin Core, Regtest
  - [x] Normal sync
  - [x] Valid transaction broadcast
  - [x] Live fork
  - [x] Fork with SQL
  - [x] Fork with a stale anchor checkpoint start
  - [x] Depth two fork, SQL
- [ ] CI
  - [x] MacOS, Windows, Linux
  - [x] 1.63, stable, beta, nightly
  - [x] Format and clippy
  - [ ] Regtest sync with Bitcoin Core
  - [x] On PR
