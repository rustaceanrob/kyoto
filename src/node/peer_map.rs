use std::{collections::BTreeMap, sync::mpsc::Sender};

use tokio::task::JoinHandle;

use crate::peers::peer::PeerError;

use super::channel_messages::MainThreadMessage;

pub(crate) struct ManagedPeer {
    main_thread_sender: Sender<MainThreadMessage>,
    handle: JoinHandle<Result<(), PeerError>>,
}

pub(crate) struct PeerMap {
    map: BTreeMap<u32, ManagedPeer>,
}
