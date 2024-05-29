use bitcoin::Transaction;
pub use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;

use super::{
    error::ClientError,
    node_messages::{ClientMessage, NodeMessage},
};

/// A [`Client`] allows for communication with a running [`Node`].
#[derive(Debug)]
pub struct Client {
    nrx: Receiver<NodeMessage>,
    ntx: Sender<ClientMessage>,
}

impl Client {
    pub(crate) fn new(nrx: Receiver<NodeMessage>, ntx: Sender<ClientMessage>) -> Self {
        Self { nrx, ntx }
    }

    /// For a majority of cases, some parts of your program will respond to node events, and other parts of the program
    /// will send events to the node. This method returns a [`ClientSender`] to issue commands to the node, and a
    /// [`Receiver`] to continually respond to events issued from the node.
    pub fn split(&mut self) -> (ClientSender, &mut Receiver<NodeMessage>) {
        (self.sender(), self.receiver())
    }

    /// Return a [`ClientSender`], which may send commands to a running node.
    pub fn sender(&self) -> ClientSender {
        ClientSender::new(self.ntx.clone())
    }

    /// Return a [`Receiver`] to listen for incoming node events.
    /// This method returns a mutable borrow to the [`Receiver`]. If you require the
    /// ability to send events to the node, i.e. to broadcast a transaction, either call [`Client::split`] or [`Client::sender`]
    /// before calling [`Client::receiver`].
    pub fn receiver(&mut self) -> &mut Receiver<NodeMessage> {
        &mut self.nrx
    }

    /// Tell the node to stop running.
    pub async fn shutdown(&mut self) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::Shutdown)
            .await
            .map_err(|_| ClientError::SendError)
    }

    /// Wait until the client's headers and filters are fully synced to connected peers, dropping any block and transaction messages.
    pub async fn wait_until_synced(&mut self) {
        loop {
            while let Some(message) = self.nrx.recv().await {
                match message {
                    NodeMessage::Synced(_) => return,
                    _ => (),
                }
            }
        }
    }

    /// Print a stream of logs to the console. This function continually loops for the duration of the program, and is not particularly helpful in production applications.
    /// See [`Client::receiver`] to listen for events from the node.
    pub async fn print_log_stream(&mut self) {
        loop {
            while let Some(message) = self.nrx.recv().await {
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
    pub async fn broadcast_tx(&mut self, tx: Transaction) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::Broadcast(tx))
            .await
            .map_err(|_| ClientError::SendError)
    }
}
