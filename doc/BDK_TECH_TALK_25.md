---
title: Guidance on integration with compact block filters
sub_title: Advice for wallet developers on building a private and slick user experience
author: rustaceanrob
theme:
  name: terminal-dark
---

# Compact block filter primer

- Block filters are a way for a light client to query if they should be interested a block without downloading the entire block. Filters are often only a couple hundred bytes.
- Filters and blocks are served by full-archival nodes directly over the Bitcoin peer-to-peer network.
- Therefore, the light client must:
  - implement a subset of the Bitcoin peer-to-peer messages
  - maintain a list of potential peers and find new ones
  - follow the header chain of most work
  - send relevant blocks to a wallet implementation

<!-- end_slide -->

# Why is this model interesting?

- The light client user is protected by the anonymity set of the entire block they are downloading
  - These blocks may also be fetched at random from the set of connections
- There are many potential peers the client may broadcast transactions to
- New features of the peer-to-peer network are more easily integrated into a CBF node
- The light client can run on resource-constrained platforms, from mobile, desktop, or a low-cost VPS (2000 sats p/m)

<!-- end_slide -->

# Reality check

- A Bitcoin node census program, developed by Spiral developer Nick Johnson, found that around 10% of 11,286 listening nodes signaled for compact block filter support
- The vast majority of the network is not listening for new connections at any given time
- This results in sync times being mostly determined in how fast a node can churn through peers in the database to find a few of these lucky peers

<!-- end_slide -->

# Potential solutions to difficulty of finding peers

- DNS seeds can be relied on more frequently for users that do not expect to be online often 
  - These seeders have up-to-date information and results may be filtered by only nodes that serve CBF
- Wallet developers can run their own set of nodes for their users, which can be used as a subset of the light client connections
  - One of the best solutions for developers that can afford to run a handful of cloud instances
- Wallets may develop an application that runs on a cheap VPS per user and stays on constantly. Users can `ssh` into the server or connect over a similar protocol
  - Easily implemented for power-users that are comfortable in the terminal
- More people can run nodes that serve compact block filters
  - Not a great solution, because incentives are low for full-archival node-runners, but still encouraged!

<!-- end_slide -->

# Another reality check

- Unconfirmed transactions cannot be verified as valid
- No elegant solution to this problem

<!-- end_slide -->

# Advice on mobile

- The modern mobile OS is very robust
  - iOS offers the scheduling of "background tasks" with the OS while your app is not open
  - While using compact block filters can be bandwidth and battery intensive for a short period, these tasks can be scheduled such that they only run on WiFi and while connected to a charger

<!-- end_slide -->

# Short example

```swift
    // When the app loads
    init() {
        // Make the OS aware of the background sync
        registerBackgroundTasks()
        // Schedule the task to run later
        scheduleBackgroundSync()
    }
```

<!-- end_slide -->

# Short example

```swift
    // Register the "handleBackgroundSync" function with the "BGTaskScheduler"
    func registerBackgroundTasks() {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: .backgroundTaskName, using: nil) {
            (task) in
            self.handleBackgroundSync(task: task as! BGProcessingTask)
        }
    }
```
<!-- end_slide -->

# Short example

```swift
    // Schedule the task to run at 3am when connected to power and WiFi
    func scheduleBackgroundSync() {
        let request = BGProcessingTaskRequest(identifier: .backgroundTaskName)
        request.requiresExternalPower = true
        request.requiresNetworkConnectivity = true
        if let next3AM = Calendar.current.date(
            bySettingHour: 3, minute: 0, second: 0, of: Date().addingTimeInterval(60)
        ) {
            request.earliestBeginDate = next3AM
        }
        do {
            try BGTaskScheduler.shared.submit(request)
        } catch  {
            #if DEBUG
            print("\(error)")
            #endif
        }
    }
```
<!-- end_slide -->

# User interface considerations

- The number of peers a client is connected to may change at any given moment
    - An icon for disconnected/connected to other nodes is a crucial component!
- Consider a progress bar showing how many filters have been downloaded out of the total
- Recover wallets with logic: there is no need to download pre-segwit filters if your user has a segwit descriptor
- Ask for recovery information: if the user knows their wallet started in 2023, don't scan for data in 2019
- New blocks gossiped to the node are instant
- Power-users can use debug logs and warnings
- The node emits information in real time and may be used to update the UI

<!-- end_slide -->

# New designs for wallets

- If users are comfortable self-hosting or paying 1000-2000 sats p/m for their wallet to be hosted on a VPS, this enables a variety of interesting experiences
    - Phones or desktops can simply connect to the remote VPS in some secure fashion, and all the wallet information with be ready and up-to-date

<!-- end_slide -->

# Future work

- Using the last `N` block(s) as data for fee estimation

# Resources

- Book of BDK tutorial: `https://bookofbdk.com/cookbook/syncing/kyoto/`
- Reference implementation: `https://github.com/2140-dev/kyoto`
- BDK integration: `https://github.com/bitcoindevkit/bdk-kyoto`
- Mobile bindings: `https://github.com/bitcoindevkit/bdk-ffi`
- Census data: `https://census.yonson.dev/`
- iOS example fork: `https://github.com/rustaceanrob/BDKSwiftExampleWallet/tree/kyoto`
- BIPs:
  - `https://github.com/bitcoin/bips/blob/master/bip-0157.mediawiki`
  - `https://github.com/bitcoin/bips/blob/master/bip-0158.mediawiki`
- Fee estimation research:
  - `https://delvingbitcoin.org/t/empirical-data-analysis-of-fee-rate-forecasters-for-asap-next-block-fee-estimation/1022`

<!-- end_slide -->
