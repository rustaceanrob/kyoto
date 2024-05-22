use bitcoin::{
    block::Header,
    p2p::{
        message_filter::{CFHeaders, CFilter, GetCFHeaders, GetCFilters},
        Address, ServiceFlags,
    },
    Block, BlockHash,
};

#[derive(Debug, Clone)]
pub(crate) enum MainThreadMessage {
    GetAddr,
    GetHeaders(GetHeaderConfig),
    GetFilterHeaders(GetCFHeaders),
    GetFilters(GetCFilters),
    GetBlock(GetBlockConfig),
    Disconnect,
    // more messages
}

#[derive(Debug, Clone)]
pub struct GetHeaderConfig {
    pub locators: Vec<BlockHash>,
    pub stop_hash: Option<BlockHash>,
}

#[derive(Debug, Clone)]
pub struct GetBlockConfig {
    pub locator: BlockHash,
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
    Filter(CFilter),
    Block(Block),
    NewBlocks(Vec<BlockHash>),
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
