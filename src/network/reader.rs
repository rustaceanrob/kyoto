use bitcoin::{
    block::Header,
    hashes::Hash,
    p2p::{
        address::AddrV2Message,
        message::NetworkMessage,
        message_blockdata::Inventory,
        message_filter::{CFHeaders, CFilter},
        message_network::VersionMessage,
        ServiceFlags,
    },
    Block, BlockHash, Transaction,
};
use bitcoin::{FeeRate, Wtxid};
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc::Sender;

use crate::messages::RejectPayload;

use super::error::ReaderError;
use super::inbound::MessageParser;
use super::MonitorGate;
use super::TimeSensitiveId;

// From Bitcoin Core PR #29575
const MAX_ADDR: usize = 1_000;
const MAX_INV: usize = 50_000;
const MAX_HEADERS: usize = 2_000;

pub(in crate::network) struct Reader<R: AsyncBufReadExt + Send + Sync + Unpin> {
    parser: MessageParser<R>,
    tx: Sender<ReaderMessage>,
    monitor_gate: MonitorGate,
}

impl<R: AsyncBufReadExt + Send + Sync + Unpin> Reader<R> {
    pub fn new(
        parser: MessageParser<R>,
        tx: Sender<ReaderMessage>,
        monitor_gate: MonitorGate,
    ) -> Self {
        Self {
            parser,
            tx,
            monitor_gate,
        }
    }

    pub(in crate::network) async fn read_from_remote(&mut self) -> Result<(), ReaderError> {
        loop {
            if let Some(message) = self.parser.read_message().await? {
                // `Inv` may carry both block and transaction announcements. Split it here so
                // `parse_message` keeps its one-message-in, one-message-out shape.
                if let NetworkMessage::Inv(inventory) = message {
                    for split in self.split_inv(inventory) {
                        self.tx.send(split).await?;
                    }
                    continue;
                }
                if let Some(cleaned) = self.parse_message(message) {
                    self.tx.send(cleaned).await?;
                }
            }
        }
    }

    fn split_inv(&self, inventory: Vec<Inventory>) -> Vec<ReaderMessage> {
        if inventory.len() > MAX_INV {
            return vec![ReaderMessage::Disconnect];
        }
        let monitoring = self.monitor_gate.is_enabled();
        let mut blocks: Vec<BlockHash> = Vec::new();
        let mut tx_wtxids: Vec<Wtxid> = Vec::new();
        for inv in inventory {
            match inv {
                Inventory::Block(hash)
                | Inventory::CompactBlock(hash)
                | Inventory::WitnessBlock(hash) => blocks.push(hash),
                Inventory::WTx(wtxid) if monitoring => tx_wtxids.push(wtxid),
                _ => (),
            }
        }
        let mut out = Vec::new();
        if !blocks.is_empty() {
            out.push(ReaderMessage::NewBlocks(blocks));
        }
        if !tx_wtxids.is_empty() {
            out.push(ReaderMessage::TxInv(tx_wtxids));
        }
        out
    }

    fn parse_message(&self, message: NetworkMessage) -> Option<ReaderMessage> {
        // Supported messages are protocol version 70013 and below
        match message {
            NetworkMessage::Version(version) => Some(ReaderMessage::Version(version)),
            NetworkMessage::Verack => Some(ReaderMessage::Verack),
            // If a peer is sending this message they are incredibly old or faulty.
            NetworkMessage::Addr(_) => None,
            // `Inv` is pre-handled in `read_from_remote` so it can emit two messages.
            NetworkMessage::Inv(_) => None,
            NetworkMessage::GetData(inventory) => Some(ReaderMessage::GetData(inventory)),
            NetworkMessage::NotFound(_) => None,
            NetworkMessage::GetBlocks(_) => None,
            NetworkMessage::GetHeaders(_) => None,
            NetworkMessage::MemPool => None,
            NetworkMessage::Tx(transaction) => self
                .monitor_gate
                .is_enabled()
                .then_some(ReaderMessage::Tx(transaction)),
            NetworkMessage::Block(block) => Some(ReaderMessage::Block(block)),
            NetworkMessage::Headers(headers) => {
                if headers.len() > MAX_HEADERS {
                    return Some(ReaderMessage::Disconnect);
                }
                Some(ReaderMessage::Headers(headers))
            }
            // 70012
            NetworkMessage::SendHeaders => None,
            NetworkMessage::GetAddr => None,
            NetworkMessage::Ping(nonce) => Some(ReaderMessage::Ping(nonce)),
            NetworkMessage::Pong(nonce) => Some(ReaderMessage::Pong(nonce)),
            NetworkMessage::MerkleBlock(_) => None,
            // Bloom Filters are enabled by 70011
            NetworkMessage::FilterLoad(_) => None,
            NetworkMessage::FilterAdd(_) => None,
            NetworkMessage::FilterClear => None,
            NetworkMessage::GetCFilters(_) => None,
            NetworkMessage::CFilter(filter) => Some(ReaderMessage::Filter(filter)),
            NetworkMessage::GetCFHeaders(_) => None,
            NetworkMessage::CFHeaders(cf_headers) => Some(ReaderMessage::FilterHeaders(cf_headers)),
            NetworkMessage::GetCFCheckpt(_) => None,
            NetworkMessage::CFCheckpt(_) => None,
            // Compact Block Relay is enabled with 70014
            NetworkMessage::SendCmpct(_) => None,
            NetworkMessage::CmpctBlock(_) => None,
            NetworkMessage::GetBlockTxn(_) => None,
            NetworkMessage::BlockTxn(_) => None,
            NetworkMessage::Alert(_) => None,
            NetworkMessage::Reject(rejection) => {
                let wtxid = Wtxid::from(rejection.hash);
                Some(ReaderMessage::Reject(RejectPayload {
                    reason: Some(rejection.ccode),
                    wtxid,
                }))
            }
            // 70013
            NetworkMessage::FeeFilter(i) => {
                if i < 0 {
                    Some(ReaderMessage::Disconnect)
                } else {
                    // Safe cast because i64::MAX < u64::MAX
                    let fee_rate = FeeRate::from_sat_per_kwu(i as u64 / 4);
                    Some(ReaderMessage::FeeFilter(fee_rate))
                }
            }
            // 70016
            NetworkMessage::WtxidRelay => None,
            NetworkMessage::AddrV2(addresses) => {
                if addresses.len() > MAX_ADDR {
                    return Some(ReaderMessage::Disconnect);
                }
                let addresses = addresses
                    .into_iter()
                    .filter(|f| {
                        f.services.has(ServiceFlags::COMPACT_FILTERS)
                            && f.services.has(ServiceFlags::NETWORK)
                    })
                    .collect::<Vec<AddrV2Message>>();
                if addresses.is_empty() {
                    return None;
                }
                Some(ReaderMessage::Addr(addresses))
            }
            NetworkMessage::SendAddrV2 => None,
            #[allow(unused)]
            NetworkMessage::Unknown { command, payload } => None,
        }
    }
}

#[derive(Debug)]
pub(in crate::network) enum ReaderMessage {
    Version(VersionMessage),
    Addr(Vec<AddrV2Message>),
    Headers(Vec<Header>),
    FilterHeaders(CFHeaders),
    Filter(CFilter),
    Block(Block),
    NewBlocks(Vec<BlockHash>),
    Reject(RejectPayload),
    Disconnect,
    Verack,
    Ping(u64),
    #[allow(dead_code)]
    Pong(u64),
    FeeFilter(FeeRate),
    GetData(Vec<Inventory>),
    TxInv(Vec<Wtxid>),
    Tx(Transaction),
}

impl ReaderMessage {
    pub(in crate::network) fn time_sensitive_message_received(&self) -> Option<TimeSensitiveId> {
        match self {
            ReaderMessage::Headers(_) => Some(TimeSensitiveId::HEADER_MSG),
            ReaderMessage::FilterHeaders(_) => Some(TimeSensitiveId::CF_HEADER_MSG),
            ReaderMessage::Filter(_) => Some(TimeSensitiveId::C_FILTER_MSG),
            ReaderMessage::Pong(_) => Some(TimeSensitiveId::PING),
            ReaderMessage::Block(b) => {
                let hash = *b.block_hash().to_raw_hash().as_byte_array();
                Some(TimeSensitiveId::from_slice(hash))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_reader(monitor_gate: MonitorGate) -> Reader<tokio::io::Empty> {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        Reader::new(
            MessageParser::V1(tokio::io::empty(), bitcoin::Network::Regtest),
            tx,
            monitor_gate,
        )
    }

    #[test]
    fn inv_split_gates_transactions_on_monitor() {
        let block = BlockHash::from_byte_array([1; 32]);
        let witness_block = BlockHash::from_byte_array([2; 32]);
        let txid = bitcoin::Txid::from_byte_array([3; 32]);
        let wtxid = bitcoin::Wtxid::from_byte_array([4; 32]);

        // Monitor off: only block hashes surface, wtxid inv is dropped.
        let gate = MonitorGate::new();
        let reader = test_reader(gate.clone());
        let split = reader.split_inv(vec![
            Inventory::Transaction(txid),
            Inventory::Block(block),
            Inventory::WTx(wtxid),
            Inventory::WitnessBlock(witness_block),
        ]);
        assert!(matches!(
            split.as_slice(),
            [ReaderMessage::NewBlocks(hashes)] if hashes == &vec![block, witness_block]
        ));
        // Transaction-only inventory produces no messages when monitoring is off.
        let split = reader.split_inv(vec![Inventory::WTx(wtxid)]);
        assert!(split.is_empty());

        // Monitor on: WTx invs surface as a separate message, alongside any blocks.
        gate.enable();
        let split = reader.split_inv(vec![
            Inventory::Block(block),
            Inventory::WTx(wtxid),
            Inventory::Transaction(txid),
        ]);
        assert!(matches!(
            split.as_slice(),
            [ReaderMessage::NewBlocks(hashes), ReaderMessage::TxInv(wtxids)]
                if hashes == &vec![block] && wtxids == &vec![wtxid]
        ));

        // Oversized inventory still disconnects.
        let oversized = vec![Inventory::Block(block); MAX_INV + 1];
        let split = reader.split_inv(oversized);
        assert!(matches!(split.as_slice(), [ReaderMessage::Disconnect]));
    }

    #[test]
    fn tx_message_gated_on_monitor() {
        let raw = hex::decode("0200000000010158e87a21b56daf0c23be8e7070456c336f7cbaa5c8757924f545887bb2abdd7501000000171600145f275f436b09a8cc9a2eb2a2f528485c68a56323feffffff02d8231f1b0100000017a914aed962d6654f9a2b36608eb9d64d2b260db4f1118700c2eb0b0000000017a914b7f5faf40e3d40a5a459b1db3535f2b72fa921e88702483045022100a22edcc6e5bc511af4cc4ae0de0fcd75c7e04d8c1c3a8aa9d820ed4b967384ec02200642963597b9b1bc22c75e9f3e117284a962188bf5e8a74c895089046a20ad770121035509a48eb623e10aace8bfd0212fdb8a8e5af3c94b0b133b95e114cab89e4f7965000000").unwrap();
        let tx: Transaction = bitcoin::consensus::deserialize(&raw).unwrap();

        let gate = MonitorGate::new();
        let reader = test_reader(gate.clone());
        assert!(reader.parse_message(NetworkMessage::Tx(tx.clone())).is_none());

        gate.enable();
        let parsed = reader.parse_message(NetworkMessage::Tx(tx.clone()));
        assert!(matches!(parsed, Some(ReaderMessage::Tx(seen)) if seen == tx));
    }
}
