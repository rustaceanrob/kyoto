use bitcoin::{
    block::Header,
    p2p::{
        address::AddrV2,
        message_filter::{CFHeaders, CFilter, GetCFHeaders, GetCFilters},
        message_network::VersionMessage,
        ServiceFlags,
    },
    Block, BlockHash, FeeRate, Transaction,
};

use crate::core::messages::FailurePayload;

#[derive(Debug, Clone)]
pub(crate) enum MainThreadMessage {
    GetAddr,
    GetAddrV2,
    GetHeaders(GetHeaderConfig),
    GetFilterHeaders(GetCFHeaders),
    GetFilters(GetCFilters),
    GetBlock(GetBlockConfig),
    Disconnect,
    BroadcastTx(Transaction),
    Verack,
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
    Version(VersionMessage),
    Addr(Vec<CombinedAddr>),
    Headers(Vec<Header>),
    FilterHeaders(CFHeaders),
    Filter(CFilter),
    Block(Block),
    NewBlocks(Vec<BlockHash>),
    Reject(FailurePayload),
    Disconnect,
    Verack,
    Ping(u64),
    #[allow(dead_code)]
    Pong(u64),
    FeeFilter(FeeRate),
}

#[derive(Debug, Clone)]
pub(crate) struct CombinedAddr {
    pub addr: AddrV2,
    pub port: u16,
    pub services: ServiceFlags,
}

impl CombinedAddr {
    pub(crate) fn new(addr: AddrV2, port: u16) -> Self {
        Self {
            addr,
            port,
            services: ServiceFlags::NONE,
        }
    }

    pub(crate) fn services(&mut self, services: ServiceFlags) {
        self.services = services
    }
}
