use tokio::sync::mpsc::{Sender, UnboundedSender};

use super::messages::{Event, Info, Warning};

#[derive(Debug, Clone)]
pub(crate) struct Dialog {
    info_tx: Sender<Info>,
    warn_tx: UnboundedSender<Warning>,
    event_tx: UnboundedSender<Event>,
}

impl Dialog {
    pub(crate) fn new(
        info_tx: Sender<Info>,
        warn_tx: UnboundedSender<Warning>,
        event_tx: UnboundedSender<Event>,
    ) -> Self {
        Self {
            info_tx,
            warn_tx,
            event_tx,
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
