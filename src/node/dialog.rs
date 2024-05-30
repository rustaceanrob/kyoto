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

    pub(crate) async fn chain_update(
        &mut self,
        num_headers: usize,
        num_cf_headers: usize,
        num_filters: usize,
        best_height: usize,
    ) {
        let message = format!(
            "Headers ({}/{}) Compact Filter Headers ({}/{}) Filters ({}/{})",
            num_headers, best_height, num_cf_headers, best_height, num_filters, best_height
        );
        let _ = self.ntx.send(NodeMessage::Dialog(message)).await;
    }

    pub(crate) async fn send_warning(&mut self, warning: String) {
        let _ = self.ntx.send(NodeMessage::Warning(warning)).await;
    }

    pub(crate) async fn send_data(&mut self, message: NodeMessage) {
        let _ = self.ntx.send(message).await;
    }
}
