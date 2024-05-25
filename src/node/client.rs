use tokio::sync::mpsc::{Receiver, Sender};

use super::node_messages::{ClientMessage, NodeMessage};

pub struct Client {
    nrx: Receiver<NodeMessage>,
    ntx: Sender<ClientMessage>,
}

impl Client {
    pub(crate) fn new(nrx: Receiver<NodeMessage>, ntx: Sender<ClientMessage>) -> Self {
        Self { nrx, ntx }
    }

    pub async fn wait_until_synced(&mut self) {
        loop {
            while let Some(message) = self.nrx.recv().await {
                match message {
                    NodeMessage::Synced => return,
                    _ => (),
                }
            }
        }
    }

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

    pub fn receiver(&mut self) -> &mut Receiver<NodeMessage> {
        &mut self.nrx
    }

    pub async fn shutdown(&mut self) {
        let _ = self.ntx.send(ClientMessage::Shutdown).await;
    }
}
