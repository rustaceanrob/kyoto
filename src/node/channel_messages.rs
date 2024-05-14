use bitcoin::{
    block::Header,
    p2p::{
        message_filter::{CFHeaders, GetCFHeaders},
        Address, ServiceFlags,
    },
    BlockHash,
};

#[derive(Debug, Clone)]
pub(crate) enum MainThreadMessage {
    GetAddr,
    GetHeaders(GetHeaderConfig),
    GetFilterHeaders(GetCFHeaders),
    Disconnect,
    // more messages
}

#[derive(Debug, Clone)]
pub struct GetHeaderConfig {
    pub locators: Vec<BlockHash>,
    pub stop_hash: Option<BlockHash>,
}

pub(crate) struct PeerThreadMessage {
    pub nonce: u32,
    pub message: PeerMessage,
}

#[derive(Debug)]

pub(crate) enum PeerMessage {
    Version(RemoteVersion),
    Addr(Vec<Address>),
    Headers(Vec<Header>),
    FilterHeaders(CFHeaders),
    Disconnect,
    Verack,
    Ping(u64),
    Pong(u64),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RemoteVersion {
    pub service_flags: ServiceFlags,
    pub timestamp: i64,
    pub height: i32,
}
