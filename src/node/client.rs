use tokio::sync::mpsc::Receiver;

use super::node_messages::NodeMessage;

pub struct Client {
    nrx: Receiver<NodeMessage>,
}

impl Client {
    pub(crate) fn new(nrx: Receiver<NodeMessage>) -> Self {
        Self { nrx }
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
}
