use tokio::sync::mpsc::{Sender, UnboundedSender};

use super::messages::{Event, Info, Warning};
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
