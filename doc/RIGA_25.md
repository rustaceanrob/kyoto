---
title: Light Clients Aren't Dead! 
sub_title: Present and future of private bitcoin use on constrained devices
author: rustaceanrob
theme:
  name: terminal-dark
---

# Hello/Background

- Studied math
- Interest in bitcoin through MIT open courseware (taught by Tadge Dryja)
- Inspired by light client space after watching SF bitdevs BIP-158 talk (by roasbeef)

<!-- end_slide -->

# Compact block filter refresher

- Block filters are a way for a light client to query if they should be interested a block without downloading the entire block.
  - Filters are often only a few hundred bytes in memory
  - Golomb coded sets are represented even more concisely over the wire
- Filters and blocks are served by full-archival nodes directly over the Bitcoin peer-to-peer network.
- Therefore, the light client must:
  - Implement a subset of the Bitcoin peer-to-peer messages
  - Maintain a list of potential peers and find new ones
    - Gossip
    - DNS
  - Follow the header chain of most work
  - Send relevant blocks to a wallet implementation

<!-- end_slide -->

# Why is this model interesting?

- The light client user is protected by the anonymity set of the entire block they are downloading
  - These blocks may also be fetched at random from the set of connections
- There are many potential peers the client may broadcast transactions to
- New features of the peer-to-peer network are more easily integrated into a CBF node
- The light client can run on resource-constrained platforms, from mobile, desktop, or a low-cost VPS (2000 sats p/m)

<!-- end_slide -->

# Demo break

<!-- end_slide -->

# Reality check

- A Bitcoin node census program, developed by Spiral developer Nick Johnson, found that around 10% of 11,286 listening nodes signaled for compact block filter support
- The vast majority of the network is not listening for new connections at any given time
  - This _should_ get better in the next release, as clearnet nodes will be listening by default
- This results in sync times being mostly determined in how fast a node can churn through peers in the database to find a few of these lucky peers
- This census runs every week and is updated here: https://census.yonson.dev/

<!-- end_slide -->

# Challenges

- Fee estimation is not possible
  - Previous outputs are required for input `OutPoint`s
  - Kind of, `Block average fee rate = (total coinbase fee - subidy) / block weight`
- Filters cannot be audited
  - The filter includes `scriptPubKey` of the tx out present in block inputs
- Silent payments requires input data
  - Shared secrets on the receiving side are computed with input public keys

<!-- end_slide -->

# Almighty undo data

- Serving the undo data directly over p2p would:
  - Enable a protocol-native silent payments light client
  - Allow clients to audit filter construction
  - Enable more robust fee estimation (transaction level estimates)
  - Enable a fully-validating SwiftSync node
- This would require another commitment scheme.

<!-- end_slide -->

# Other improvements

- Current filters are wire efficient, but perhaps not CPU efficent:
  - One per block with unique keys
  - Uses siphash
- New filter types could be introduced that
  - Represent a range of blocks
    - Reduce the number of hashes to `1/N` blocks
  - Use CPU accelerated hashes

<!-- end_slide -->

# Work in reducing the altruism

- Clearnet will listen for connections by default in the next Bitcoin Core release
- An index can make rescans much faster
- Ongoing work to make building an index much faster

<!-- end_slide -->

# 2140

- What is it
- What I work on
- What you can do

# Contact

- hi: `rob@2140.dev`
- github: `rustaceanrob`
- rss: `https://rustaceanrob.com`

<!-- end_slide -->

# Resources

#### Development

- Book of BDK tutorial: `https://bookofbdk.com/cookbook/syncing/kyoto/`
- Reference implementation: `https://github.com/2140-dev/kyoto`
- Mobile bindings: `https://github.com/bitcoindevkit/bdk-ffi`

#### Research

- Multi-set multi-membership query: `https://sci-hub.se/10.1145/3448016.3452829`
- Cuckoo filters foundation: `https://www.cs.cmu.edu/~dga/papers/cuckoo-conext2014.pdf`
- Efficiently sized quotient filters: `https://dl.acm.org/doi/pdf/10.1145/3035918.3035963`

#### Apps

- Your app!
- BDK Swift example iOS wallet: `https://github.com/bitcoindevkit/BDKSwiftExampleWallet`
- Amstel macOS wallet: `https://github.com/rustaceanrob/amstel`

#### etc

- MIT opencourseware: `https://www.youtube.com/playlist?list=PL1fwlbeJALZw1OJ3kIdWiOty9IHyMjjVq`
- SF Bitdevs: `https://youtu.be/7FWKc8lM4Ek?si=4YZHn-wZXpzW9SXA`
- Census data: `https://census.yonson.dev/`
- BIPs:
  - `https://github.com/bitcoin/bips/blob/master/bip-0157.mediawiki`
  - `https://github.com/bitcoin/bips/blob/master/bip-0158.mediawiki`

<!-- end_slide -->
