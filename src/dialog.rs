use tokio::sync::mpsc::{Sender, UnboundedSender};

use super::messages::{Event, Info, Progress, Warning};
use crate::LogLevel;

#[derive(Debug, Clone)]
pub(crate) struct Dialog {
    pub(crate) log_level: LogLevel,
    log_tx: Sender<String>,
    info_tx: Sender<Info>,
    warn_tx: UnboundedSender<Warning>,
    event_tx: UnboundedSender<Event>,
}

impl Dialog {
    pub(crate) fn new(
        log_level: LogLevel,
        log_tx: Sender<String>,
        info_tx: Sender<Info>,
        warn_tx: UnboundedSender<Warning>,
        event_tx: UnboundedSender<Event>,
    ) -> Self {
        Self {
            log_level,
            log_tx,
            info_tx,
            warn_tx,
            event_tx,
        }
    }

    pub(crate) async fn send_dialog(&self, dialog: impl Into<String>) {
        let _ = self.log_tx.send(dialog.into()).await;
    }

    pub(crate) async fn chain_update(
        &self,
        num_headers: u32,
        num_cf_headers: u32,
        num_filters: u32,
        best_height: u32,
    ) {
        if matches!(self.log_level, LogLevel::Debug | LogLevel::Info) {
            let _ = self
                .info_tx
                .send(Info::Progress(Progress::new(
                    num_cf_headers,
                    num_filters,
                    best_height,
                )))
                .await;
        }
        if matches!(self.log_level, LogLevel::Debug) {
            let message = format!(
                "Headers ({}/{}) Compact Filter Headers ({}/{}) Filters ({}/{})",
                num_headers, best_height, num_cf_headers, best_height, num_filters, best_height
            );
            let _ = self.log_tx.send(message).await;
        }
    }

    pub(crate) fn send_warning(&self, warning: Warning) {
        let _ = self.warn_tx.send(warning);
    }

    pub(crate) async fn send_info(&self, info: Info) {
        let _ = self.info_tx.send(info).await;
    }

    pub(crate) fn send_event(&self, message: Event) {
        let _ = self.event_tx.send(message);
    }
}
