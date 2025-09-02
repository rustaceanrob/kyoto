# Iterations on current work

After integration with BDK, particularly the BDK bindings, there is a better picutre of how a CBF client works in practice.

## Block headers

The purpose of the block header storage is to 1. inform BDK, or any other client, about re-orgs 2. audit the difficulty adjustment 3. request filters for a range.

If we assume BDK can initialize with some recent headers at startup, which appears to be the case in https://github.com/bitcoindevkit/bdk/pull/1582/files, then there is no need to store any block headers on the CBF client side. The CBF client can emit any header updates or reorgs, and all changes may be stored locally by the wallet. For higher security guarantees, the wallet may simply supply a longer history at startup to audit the difficulty.

## Sqlite

The bindings to sqlite are difficult to work with, as only one version of sqlite may be used across _all_ dependencies. Without header storage, simple flat-file storage of peers allows the removal of `rusqlite` entirely. This new implementation of peer storage may emulate Bitcoin Core's bucketing strategy, which will hopefully allow for faster peer-polling.

ref: https://github.com/bitcoin-core/bitcoin-devwiki/wiki/Addrman-and-eclipse-attacks

## Peers

One thing that has become increasingly clear is the need for a task to poll peers for their liveliness via feeler connections. Every `N` seconds, some new/tried peer should be polled to see if they are alive. This should greatly improve the selection process.

## Async pain

The primary pain with `async` has come in the form of the bindings. All structs that use UniFFI must be immutable, which leads to everything being wrapped in a `std::sync::Mutex`. Unfortunately, locks cannot be held across `await` points, which makes the current API difficult to work with.

In practice, most users only connect to a few peers as well. In some cases, it will be easier to use `std::thread` instead of the async driver. Future versions should have a "pick your own driver" interface, of which most I/O will be occurring on the P2P side.

## Logging

In the first iteration a goal was to keep logging completely up to the user. In practice, this has led to some foot-guns. Particularly, I have seen users parsing the logs directly for information, which is not great at all for performance, and also implies the logs have to be _versioned_ somehow. Also, by emitting the string logs via a channel, every log emission becomes `async`, which generally does not make much sense. Execution shouldn't yield just to print a string. Next steps would be to implement an in-house struct/trait to handle log strings. I think `Info`, `Event` etc will remain channels, so as to separate data and string types.
