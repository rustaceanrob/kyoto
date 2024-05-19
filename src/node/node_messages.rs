#[derive(Debug)]
pub(crate) enum NodeMessage {
    Headers,
    Synced,
}

#[derive(Debug)]
pub(crate) enum ClientMessage {
    Shutdown,
}
