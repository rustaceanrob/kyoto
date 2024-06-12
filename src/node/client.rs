use std::collections::HashSet;

use bitcoin::ScriptBuf;
use tokio::sync::broadcast;
pub use tokio::sync::broadcast::Receiver;
pub use tokio::sync::mpsc::Sender;

use crate::{IndexedBlock, IndexedTransaction, TxBroadcast};

use super::{
    error::ClientError,
    messages::{ClientMessage, NodeMessage},
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
    pub fn split(&mut self) -> (ClientSender, Receiver<NodeMessage>) {
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
    pub fn receiver(&mut self) -> Receiver<NodeMessage> {
        self.nrx.subscribe()
    }

    /// Tell the node to stop running.
    pub async fn shutdown(&mut self) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::Shutdown)
            .await
            .map_err(|_| ClientError::SendError)
    }

    /// Collect the transctions received from the node into an in-memory cache,
    /// returning once the node is synced to its peers.
    /// To instead respond to transactions or blocks as the node receives them,
    /// see [`Client::receiver`]. Responding to events is more appropriate for implementations
    /// such as Lightning Network nodes, which are expected to be online for long durations.
    pub async fn collect_relevant_tx(&mut self) -> Vec<IndexedTransaction> {
        let mut txs = Vec::new();
        let mut rec = self.nrx.subscribe();
        loop {
            while let Ok(message) = rec.recv().await {
                match message {
                    NodeMessage::Transaction(tx) => txs.push(tx),
                    NodeMessage::Synced(_) => {
                        drop(rec);
                        return txs;
                    }
                    _ => (),
                }
            }
        }
    }

    /// Collect the blocks received from the node into an in-memory cache,
    /// returning once the node is synced to its peers.
    /// Only recommended for machines that can tolerate such a memory allocation,
    /// such as a server or desktop computer.
    /// For devices like smart phones, see [`Client::collect_relevant_tx`].
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

    /// Wait until the client's headers and filters are fully synced to connected peers, dropping any block and transaction messages.
    pub async fn wait_until_synced(&mut self) {
        let mut rec = self.nrx.subscribe();
        loop {
            while let Ok(message) = rec.recv().await {
                if let NodeMessage::Synced(_) = message {
                    drop(rec);
                    return;
                }
            }
        }
    }

    /// Print a stream of logs to the console. This function continually loops for the duration of the program, and is not particularly helpful in production applications.
    /// See [`Client::receiver`] to listen for events from the node.
    pub async fn print_log_stream(&mut self) {
        let mut rec = self.nrx.subscribe();
        loop {
            while let Ok(message) = rec.recv().await {
                match message {
                    NodeMessage::Dialog(message) => {
                        println!("\x1b[32mInfo\x1b[0m {}", message);
                    }
                    NodeMessage::Warning(message) => {
                        println!("\x1b[93mWarn\x1b[0m {}", message);
                    }
                    _ => (),
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
    pub async fn shutdown(&mut self) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::Shutdown)
            .await
            .map_err(|_| ClientError::SendError)
    }

    /// Broadcast a new transaction to the network.
    pub async fn broadcast_tx(&mut self, tx: TxBroadcast) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::Broadcast(tx))
            .await
            .map_err(|_| ClientError::SendError)
    }

    /// Add more Bitcoin [`ScriptBuf`] to watch for. Does not rescan the filters.
    pub async fn add_scripts(&mut self, scripts: HashSet<ScriptBuf>) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::AddScripts(scripts))
            .await
            .map_err(|_| ClientError::SendError)
    }
}
