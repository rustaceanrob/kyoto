use tokio::sync::mpsc::{Sender, UnboundedSender};

use super::messages::{Event, Log, Progress, Warning};

#[derive(Debug, Clone)]
pub(crate) struct Dialog {
    log_tx: Sender<Log>,
    warn_tx: UnboundedSender<Warning>,
    event_tx: UnboundedSender<Event>,
}

impl Dialog {
    pub(crate) fn new(
        log_tx: Sender<Log>,
        warn_tx: UnboundedSender<Warning>,
        event_tx: UnboundedSender<Event>,
    ) -> Self {
        Self {
            log_tx,
            warn_tx,
            event_tx,
        }
    }

    pub(crate) async fn send_dialog(&self, dialog: impl Into<String>) {
        let _ = self.log_tx.send(Log::Dialog(dialog.into())).await;
    }

    pub(crate) async fn chain_update(
        &self,
        num_headers: u32,
        num_cf_headers: u32,
        num_filters: u32,
        best_height: u32,
    ) {
        let _ = self
            .log_tx
            .send(Log::Progress(Progress::new(
                num_cf_headers,
                num_filters,
                best_height,
            )))
            .await;
        let message = format!(
            "Headers ({}/{}) Compact Filter Headers ({}/{}) Filters ({}/{})",
            num_headers, best_height, num_cf_headers, best_height, num_filters, best_height
        );
        let _ = self.log_tx.send(Log::Dialog(message)).await;
    }

    pub(crate) async fn send_warning(&self, warning: Warning) {
        let _ = self.warn_tx.send(warning);
    }

    pub(crate) async fn send_info(&self, info: Log) {
        let _ = self.log_tx.send(info).await;
    }

    pub(crate) async fn send_event(&self, message: Event) {
        let _ = self.event_tx.send(message);
    }
}
