use bip324::serde::NetworkMessage;
use bip324::PacketReader;
use bitcoin::consensus::{deserialize, deserialize_partial};
use bitcoin::p2p::message::RawNetworkMessage;
use bitcoin::Network;
use tokio::io::AsyncReadExt;

use crate::prelude::FutureResult;

use super::error::PeerReadError;
use super::traits::{MessageParser, StreamReader};
use super::V1Header;

const MAX_MESSAGE_BYTES: u32 = 1024 * 1024 * 32;
const DECOY_BYTE: u8 = 128;

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

pub(crate) struct V2MessageParser {
    stream: StreamReader,
    decryptor: PacketReader,
}

impl V2MessageParser {
    pub(crate) fn new(stream: StreamReader, decryptor: PacketReader) -> Self {
        Self { stream, decryptor }
    }

    async fn do_read_message(&mut self) -> Result<Option<NetworkMessage>, PeerReadError> {
        let mut stream = self.stream.lock().await;
        let mut len_buf = [0; 3];
        let _ = stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|_| PeerReadError::ReadBuffer)?;
        let message_len = self.decryptor.decypt_len(len_buf);
        if message_len > MAX_MESSAGE_BYTES as usize {
            return Err(PeerReadError::TooManyMessages);
        }
        let mut response_message = vec![0; message_len];
        let _ = stream
            .read_exact(&mut response_message)
            .await
            .map_err(|_| PeerReadError::ReadBuffer)?;
        let msg = self
            .decryptor
            .decrypt_contents_with_alloc(&response_message, None)
            .map_err(|_| PeerReadError::DecryptionFailed)?;
        let header = msg.first().ok_or(PeerReadError::ReadBuffer)?;
        if DECOY_BYTE.eq(header) {
            Ok(None)
        } else {
            let parsed = bip324::serde::deserialize(&msg[1..])
                .map_err(|_| PeerReadError::Deserialization)?;
            Ok(Some(parsed))
        }
    }
}

impl MessageParser for V2MessageParser {
    fn read_message(&mut self) -> FutureResult<Option<NetworkMessage>, PeerReadError> {
        Box::pin(self.do_read_message())
    }
}
