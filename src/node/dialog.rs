use tokio::sync::broadcast::Sender;

use super::messages::NodeMessage;

#[derive(Debug, Clone)]
pub(crate) struct Dialog {
    ntx: Sender<NodeMessage>,
}

impl Dialog {
    pub(crate) fn new(ntx: Sender<NodeMessage>) -> Self {
        Self { ntx }
    }

    pub(crate) async fn send_dialog(&self, dialog: String) {
        let _ = self.ntx.send(NodeMessage::Dialog(dialog));
    }

    pub(crate) async fn chain_update(
        &self,
        num_headers: u32,
        num_cf_headers: u32,
        num_filters: u32,
        best_height: usize,
    ) {
        let message = format!(
            "Headers ({}/{}) Compact Filter Headers ({}/{}) Filters ({}/{})",
            num_headers, best_height, num_cf_headers, best_height, num_filters, best_height
        );
        let _ = self.ntx.send(NodeMessage::Dialog(message));
    }

    pub(crate) async fn send_warning(&self, warning: String) {
        let _ = self.ntx.send(NodeMessage::Warning(warning));
    }

    pub(crate) async fn send_data(&self, message: NodeMessage) {
        let _ = self.ntx.send(message);
    }
}
