use std::collections::HashSet;

#[cfg(feature = "silent-payments")]
use bitcoin::BlockHash;
use bitcoin::ScriptBuf;
use tokio::sync::broadcast;
pub use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;

use crate::{IndexedBlock, TxBroadcast};

use super::{
    error::ClientError,
    messages::{ClientMessage, NodeMessage, SyncUpdate},
};

/// A [`Client`] allows for communication with a running node.
#[derive(Debug, Clone)]
pub struct Client {
    nrx: broadcast::Sender<NodeMessage>,
    ntx: Sender<ClientMessage>,
}

impl Client {
    pub(crate) fn new(nrx: broadcast::Sender<NodeMessage>, ntx: Sender<ClientMessage>) -> Self {
        Self { nrx, ntx }
    }

    /// For a majority of cases, some parts of your program will respond to node events, and other parts of the program
    /// will send events to the node. This method returns a [`ClientSender`] to issue commands to the node, and a
    /// [`Receiver`] to continually respond to events issued from the node.
    pub fn split(&self) -> (ClientSender, Receiver<NodeMessage>) {
        (self.sender(), self.receiver())
    }

    /// Return a [`ClientSender`], which may send commands to a running node.
    pub fn sender(&self) -> ClientSender {
        ClientSender::new(self.ntx.clone())
    }

    /// Return a [`Receiver`] to listen for incoming node events.
    /// You may call this function as many times as required, however please note
    /// there are memory and performance implications when calling this method. Namely, a clone of the object,
    /// potentially a large data structure like a [`crate::Block`], is held in memory for _every_ receiver until _all_
    /// receivers have gotten the message.
    /// You should only call this twice if two separate portions of your application need to process
    /// data differently. For example, a Lightning Network node implementation.
    pub fn receiver(&self) -> Receiver<NodeMessage> {
        self.nrx.subscribe()
    }

    /// Collect the blocks received from the node into an in-memory cache,
    /// returning once the node is synced to its peers.
    /// Only recommended for machines that can tolerate such a memory allocation,
    /// like a server or desktop computer. Internally, this method also creates a new receiver,
    /// which has more performance implications.
    /// For devices like smart phones, it is advisable to
    /// respond to each block in an event loop. See [`Client::split`] or [`Client::receiver`].
    pub async fn collect_relevant_blocks(&mut self) -> Vec<IndexedBlock> {
        let mut rec = self.nrx.subscribe();
        let mut blocks = Vec::new();
        loop {
            while let Ok(message) = rec.recv().await {
                match message {
                    NodeMessage::Block(block) => blocks.push(block),
                    NodeMessage::Synced(_) => {
                        drop(rec);
                        return blocks;
                    }
                    _ => (),
                }
            }
        }
    }

    /// Broadcast a transaction and wait for the transaction to be sent to at least one node.
    /// Note that this may take a significant amount of time depending on your peer and broadcast policy.
    /// Furthermore, this is not a guarantee that the transaction has been _propagated_ throughout the network,
    /// only that one or more peers has received the transaction.
    ///
    /// # Errors
    ///
    /// If the node has stopped running or the transaction was rejected by at least one peer.
    pub async fn wait_for_broadcast(&mut self, tx: TxBroadcast) -> Result<(), ClientError> {
        let txid = tx.tx.compute_txid();
        self.ntx
            .send(ClientMessage::Broadcast(tx))
            .await
            .map_err(|_| ClientError::SendError)?;
        let mut rec = self.nrx.subscribe();
        loop {
            while let Ok(message) = rec.recv().await {
                match message {
                    NodeMessage::TxSent(id) => {
                        if id.eq(&txid) {
                            return Ok(());
                        }
                    }
                    NodeMessage::TxBroadcastFailure(id) => {
                        if id.txid.eq(&txid) {
                            return Err(ClientError::BroadcastFailure(id));
                        }
                    }
                    _ => continue,
                }
            }
        }
    }

    /// Wait until the client's headers and filters are fully synced to connected peers, dropping any block and transaction messages.
    pub async fn wait_until_synced(&mut self) -> SyncUpdate {
        let mut rec = self.nrx.subscribe();
        loop {
            while let Ok(message) = rec.recv().await {
                if let NodeMessage::Synced(update) = message {
                    return update;
                }
            }
        }
    }
}

/// Send messages to a node that is running so the node may complete a task.
#[derive(Debug, Clone)]
pub struct ClientSender {
    ntx: Sender<ClientMessage>,
}

impl ClientSender {
    fn new(ntx: Sender<ClientMessage>) -> Self {
        Self { ntx }
    }
}

macro_rules! impl_core_client {
    ($client:ident) => {
        impl $client {
            /// Tell the node to shut down.
            ///
            /// # Errors
            ///
            /// If the node has already stopped running.
            pub async fn shutdown(&self) -> Result<(), ClientError> {
                self.ntx
                    .send(ClientMessage::Shutdown)
                    .await
                    .map_err(|_| ClientError::SendError)
            }

            /// Broadcast a new transaction to the network.
            ///
            /// # Errors
            ///
            /// If the node has stopped running.
            pub async fn broadcast_tx(&self, tx: TxBroadcast) -> Result<(), ClientError> {
                self.ntx
                    .send(ClientMessage::Broadcast(tx))
                    .await
                    .map_err(|_| ClientError::SendError)
            }

            /// Add more Bitcoin [`ScriptBuf`] to watch for. Does not rescan the filters.
            ///
            /// # Errors
            ///
            /// If the node has stopped running.
            #[cfg(not(feature = "silent-payments"))]
            pub async fn add_scripts(
                &self,
                scripts: HashSet<ScriptBuf>,
            ) -> Result<(), ClientError> {
                self.ntx
                    .send(ClientMessage::AddScripts(scripts))
                    .await
                    .map_err(|_| ClientError::SendError)
            }

            /// Starting at the configured anchor checkpoint, look for block inclusions with newly added scripts.
            ///
            /// # Errors
            ///
            /// If the node has stopped running.
            pub async fn rescan(&self) -> Result<(), ClientError> {
                self.ntx
                    .send(ClientMessage::Rescan)
                    .await
                    .map_err(|_| ClientError::SendError)
            }

            /// Explicitly start the block filter syncing process. Note that the node will automatically download and check
            /// filters unless the policy is to explicitly halt.
            ///
            /// # Errors
            ///
            /// If the node has stopped running.
            pub async fn continue_download(&self) -> Result<(), ClientError> {
                self.ntx
                    .send(ClientMessage::ContinueDownload)
                    .await
                    .map_err(|_| ClientError::SendError)
            }

            /// Request a block be fetched.
            ///
            /// # Errors
            ///
            /// If the node has stopped running.
            #[cfg(feature = "silent-payments")]
            pub async fn get_block(&self, block_hash: BlockHash) -> Result<(), ClientError> {
                self.ntx
                    .send(ClientMessage::GetBlock(block_hash))
                    .await
                    .map_err(|_| ClientError::SendError)
            }
        }
    };
}

impl_core_client!(Client);
impl_core_client!(ClientSender);

#[cfg(test)]
mod tests {
    use bitcoin::{consensus::deserialize, Transaction};
    use tokio::sync::mpsc;

    use super::*;

    #[tokio::test]
    async fn test_client_works() {
        let transaction: Transaction = deserialize(&hex::decode("0200000001aad73931018bd25f84ae400b68848be09db706eac2ac18298babee71ab656f8b0000000048473044022058f6fc7c6a33e1b31548d481c826c015bd30135aad42cd67790dab66d2ad243b02204a1ced2604c6735b6393e5b41691dd78b00f0c5942fb9f751856faa938157dba01feffffff0280f0fa020000000017a9140fb9463421696b82c833af241c78c17ddbde493487d0f20a270100000017a91429ca74f8a08f81999428185c97b5d852e4063f618765000000").unwrap()).unwrap();
        let (ntx, _) = broadcast::channel::<NodeMessage>(32);
        let (ctx, crx) = mpsc::channel::<ClientMessage>(5);
        let client = Client::new(ntx.clone(), ctx);
        let mut recv = client.receiver();
        let send_res = ntx.send(NodeMessage::Dialog("An important message".into()));
        assert!(send_res.is_ok());
        let message = recv.recv().await;
        assert!(message.is_ok());
        tokio::task::spawn(async move {
            ntx.send(NodeMessage::Dialog("Another important message".into()))
        });
        assert!(send_res.is_ok());
        let message = recv.recv().await;
        assert!(message.is_ok());
        drop(recv);
        let broadcast = client
            .broadcast_tx(TxBroadcast::new(
                transaction.clone(),
                crate::TxBroadcastPolicy::AllPeers,
            ))
            .await;
        assert!(broadcast.is_ok());
        drop(crx);
        let broadcast = client.shutdown().await;
        assert!(broadcast.is_err());
    }
}
