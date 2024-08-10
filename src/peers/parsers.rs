use bip324::serde::NetworkMessage;
use bitcoin::consensus::{deserialize, deserialize_partial};
use bitcoin::p2p::message::RawNetworkMessage;
use bitcoin::Network;
use tokio::io::AsyncReadExt;

use crate::prelude::FutureResult;

use super::error::PeerReadError;
use super::traits::{MessageParser, StreamReader};
use super::V1Header;

const MAX_MESSAGE_BYTES: u32 = 1024 * 1024 * 32;

pub(crate) struct V1MessageParser {
    stream: StreamReader,
    network: Network,
}

impl V1MessageParser {
    pub(crate) fn new(stream: StreamReader, network: Network) -> Self {
        Self { stream, network }
    }

    async fn do_read_message(&mut self) -> Result<Option<NetworkMessage>, PeerReadError> {
        let mut stream = self.stream.lock().await;
        let mut message_buf = vec![0_u8; 24];
        let _ = stream
            .read_exact(&mut message_buf)
            .await
            .map_err(|_| PeerReadError::ReadBuffer)?;
        let header: V1Header = deserialize_partial(&message_buf)
            .map_err(|_| PeerReadError::Deserialization)?
            .0;
        // Nonsense for our network
        if header.magic != self.network.magic() {
            return Err(PeerReadError::Deserialization);
        }
        // Message is too long
        if header.length > MAX_MESSAGE_BYTES {
            return Err(PeerReadError::Deserialization);
        }
        let mut contents_buf = vec![0_u8; header.length as usize];
        let _ = stream
            .read_exact(&mut contents_buf)
            .await
            .map_err(|_| PeerReadError::ReadBuffer)?;
        message_buf.extend_from_slice(&contents_buf);
        let message: RawNetworkMessage =
            deserialize(&message_buf).map_err(|_| PeerReadError::Deserialization)?;
        Ok(Some(message.payload().clone()))
    }
}

impl MessageParser for V1MessageParser {
    fn read_message(&mut self) -> FutureResult<Option<NetworkMessage>, PeerReadError> {
        Box::pin(self.do_read_message())
    }
}
