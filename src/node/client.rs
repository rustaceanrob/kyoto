use std::collections::HashSet;

use bitcoin::ScriptBuf;
use tokio::sync::broadcast;
pub use tokio::sync::broadcast::Receiver;
pub use tokio::sync::mpsc::Sender;

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

    /// Tell the node to stop running.
    pub async fn shutdown(&self) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::Shutdown)
            .await
            .map_err(|_| ClientError::SendError)
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

    /// Tell the node to shut down.
    pub async fn shutdown(&self) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::Shutdown)
            .await
            .map_err(|_| ClientError::SendError)
    }

    /// Broadcast a new transaction to the network.
    pub async fn broadcast_tx(&self, tx: TxBroadcast) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::Broadcast(tx))
            .await
            .map_err(|_| ClientError::SendError)
    }

    /// Add more Bitcoin [`ScriptBuf`] to watch for. Does not rescan the filters.
    pub async fn add_scripts(&self, scripts: HashSet<ScriptBuf>) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::AddScripts(scripts))
            .await
            .map_err(|_| ClientError::SendError)
    }

    /// Starting at the configured anchor checkpoint, look for block inclusions with newly added scripts.
    pub async fn rescan(&self) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::Rescan)
            .await
            .map_err(|_| ClientError::SendError)
    }
}
