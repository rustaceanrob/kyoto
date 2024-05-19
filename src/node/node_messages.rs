#[derive(Debug)]
pub(crate) enum NodeMessage {
    Dialog(String),
    Synced,
}

#[derive(Debug)]
pub(crate) enum ClientMessage {
    Shutdown,
}
