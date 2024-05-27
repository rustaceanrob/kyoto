use tokio::sync::mpsc::Sender;

use super::node_messages::NodeMessage;

#[derive(Debug, Clone)]
pub(crate) struct Dialog {
    ntx: Sender<NodeMessage>,
}

impl Dialog {
    pub(crate) fn new(ntx: Sender<NodeMessage>) -> Self {
        Self { ntx }
    }

    pub(crate) async fn send_dialog(&mut self, dialog: String) {
        let _ = self.ntx.send(NodeMessage::Dialog(dialog)).await;
    }

    pub(crate) async fn send_warning(&mut self, warning: String) {
        let _ = self.ntx.send(NodeMessage::Warning(warning)).await;
    }

    pub(crate) async fn send_data(&mut self, message: NodeMessage) {
        let _ = self.ntx.send(message).await;
    }
}
