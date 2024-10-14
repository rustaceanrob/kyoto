extern crate tokio;
use std::{ops::DerefMut, time::Duration};

use bip324::{Handshake, PacketHandler, Role};
use bitcoin::{p2p::ServiceFlags, Network};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    select,
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
    time::Instant,
};

use crate::{
    core::{
        channel_messages::{MainThreadMessage, PeerMessage, PeerThreadMessage},
        dialog::Dialog,
        messages::Warning,
    },
    network::outbound_messages::V1OutboundMessage,
};

use super::{
    counter::MessageCounter,
    error::PeerError,
    parsers::V1MessageParser,
    reader::Reader,
    traits::{MessageGenerator, StreamReader, StreamWriter},
};

#[cfg(not(feature = "tor"))]
use super::outbound_messages::V2OutboundMessage;
#[cfg(not(feature = "tor"))]
use super::parsers::V2MessageParser;

const MESSAGE_TIMEOUT: u64 = 2;
const HANDSHAKE_TIMEOUT: u64 = 2;
//                    sec  min  hour
const TWO_HOUR: u64 = 60 * 60 * 2;
const MAX_RESPONSE: [u8; 4111] = [0; 4111];

type MutexMessageGenerator = Mutex<Box<dyn MessageGenerator>>;

pub(crate) struct Peer {
    nonce: u32,
    main_thread_sender: Sender<PeerThreadMessage>,
    main_thread_recv: Receiver<MainThreadMessage>,
    network: Network,
    message_counter: MessageCounter,
    services: ServiceFlags,
    dialog: Dialog,
}

impl Peer {
    pub fn new(
        nonce: u32,
        network: Network,
        main_thread_sender: Sender<PeerThreadMessage>,
        main_thread_recv: Receiver<MainThreadMessage>,
        services: ServiceFlags,
        dialog: Dialog,
        timeout: Duration,
    ) -> Self {
        let message_counter = MessageCounter::new(timeout);
        Self {
            nonce,
            main_thread_sender,
            main_thread_recv,
            network,
            message_counter,
            services,
            dialog,
        }
    }

    pub async fn run(
        &mut self,
        reader: StreamReader,
        writer: StreamWriter,
    ) -> Result<(), PeerError> {
        let start_time = Instant::now();
        let (tx, mut rx) = mpsc::channel(32);
        let mut lock = writer.lock().await;
        let writer = lock.deref_mut();

        // If a peer signals for V2 we will use it, otherwise just use plaintext.
        #[cfg(not(feature = "tor"))]
        let (message_mutex, mut peer_reader) = if self.services.has(ServiceFlags::P2P_V2) {
            let mut lock = reader.lock().await;
            let read_lock = lock.deref_mut();
            let handshake_result = tokio::time::timeout(
                Duration::from_secs(HANDSHAKE_TIMEOUT),
                self.try_handshake(writer, read_lock),
            )
            .await
            .map_err(|_| PeerError::HandshakeFailed)?;
            if let Err(ref e) = handshake_result {
                self.dialog
                    .send_dialog(format!("Failed to establish an encrypted connection: {e}"))
                    .await;
                self.dialog.send_warning(Warning::CouldNotConnect).await;
            }
            let packet_handler = handshake_result?;
            let (decryptor, encryptor) = packet_handler.into_split();
            let message_mutex: MutexMessageGenerator =
                Mutex::new(Box::new(V2OutboundMessage::new(self.network, encryptor)));
            drop(lock);
            let reader = Reader::new(V2MessageParser::new(reader, decryptor), tx);
            (message_mutex, reader)
        } else {
            let outbound_messages = V1OutboundMessage::new(self.network);
            let message_mutex: MutexMessageGenerator = Mutex::new(Box::new(outbound_messages));
            let reader = Reader::new(V1MessageParser::new(reader, self.network), tx);
            (message_mutex, reader)
        };

        // V2 handshakes fail frequently over Tor and messages are encrypted over relays anyway.
        #[cfg(feature = "tor")]
        let (message_mutex, mut peer_reader) = {
            let outbound_messages = V1OutboundMessage::new(self.network);
            let message_mutex: MutexMessageGenerator = Mutex::new(Box::new(outbound_messages));
            let reader = Reader::new(V1MessageParser::new(reader, self.network), tx);
            (message_mutex, reader)
        };

        let mut message_lock = message_mutex.lock().await;
        let outbound_messages = message_lock.deref_mut();
        let message = outbound_messages.version_message(None)?;
        self.write_bytes(writer, message).await?;
        self.message_counter.sent_version();
        let read_handle = tokio::spawn(async move {
            peer_reader
                .read_from_remote()
                .await
                .map_err(|_| PeerError::Reader)
        });
        loop {
            if read_handle.is_finished() {
                return Ok(());
            }
            if self.message_counter.unsolicited() {
                self.dialog.send_warning(Warning::UnsolicitedMessage).await;
                return Ok(());
            }
            if self.message_counter.unresponsive() {
                self.dialog.send_warning(Warning::PeerTimedOut).await;
                return Ok(());
            }
            if Instant::now().duration_since(start_time) > Duration::from_secs(TWO_HOUR) {
                self.dialog
                    .send_dialog(format!(
                        "The connection to peer {} has been maintained for over {} seconds, finding a new peer",
                        self.nonce, TWO_HOUR,
                    ))
                    .await;
                return Ok(());
            }
            select! {
                // The peer sent us a message
                peer_message = tokio::time::timeout(Duration::from_secs(MESSAGE_TIMEOUT), rx.recv()) => {
                    if let Ok(peer_message) = peer_message {
                        match peer_message {
                            Some(message) => {
                                match self.handle_peer_message(message, writer, outbound_messages).await {
                                    Ok(()) => continue,
                                    Err(e) => {
                                        match e {
                                            // We were told by the reader thread to disconnect from this peer
                                            PeerError::DisconnectCommand => return Ok(()),
                                            _ => continue,
                                        }
                                    },
                                }
                            },
                            None => continue,
                        }
                    }
                }
                // The main thread sent us a message
                node_message = self.main_thread_recv.recv() => {
                    match node_message {
                        Some(message) => {
                            match self.main_thread_request(message, writer, outbound_messages).await {
                                Ok(()) => continue,
                                Err(e) => {
                                    match e {
                                        // We were told by the main thread to disconnect from this peer
                                        PeerError::DisconnectCommand => return Ok(()),
                                        _ => continue,
                                    }
                                },
                            }
                        },
                        None => continue,
                    }
                }
            }
        }
    }

    async fn handle_peer_message<W>(
        &mut self,
        message: PeerMessage,
        writer: &mut W,
        message_generator: &mut Box<dyn MessageGenerator>,
    ) -> Result<(), PeerError>
    where
        W: AsyncWrite + Send + Unpin,
    {
        match message {
            PeerMessage::Version(version) => {
                self.message_counter.got_version();
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::Version(version),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                Ok(())
            }
            PeerMessage::Addr(addrs) => {
                self.message_counter.got_addrs(addrs.len());
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::Addr(addrs),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                Ok(())
            }
            PeerMessage::Headers(headers) => {
                self.message_counter.got_header();
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::Headers(headers),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                Ok(())
            }
            PeerMessage::FilterHeaders(cf_headers) => {
                self.message_counter.got_filter_header();
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::FilterHeaders(cf_headers),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                Ok(())
            }
            PeerMessage::Filter(filter) => {
                self.message_counter.got_filter();
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::Filter(filter),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                Ok(())
            }
            PeerMessage::Block(block) => {
                self.message_counter.got_block();
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::Block(block),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                Ok(())
            }
            PeerMessage::NewBlocks(block_hashes) => {
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::NewBlocks(block_hashes),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                Ok(())
            }
            PeerMessage::Verack => {
                self.message_counter.got_verack();
                Ok(())
            }
            PeerMessage::Ping(nonce) => {
                let message = message_generator.pong(nonce)?;
                self.write_bytes(writer, message).await?;
                Ok(())
            }
            PeerMessage::Pong(_) => Ok(()),
            PeerMessage::Reject(payload) => {
                self.message_counter.got_reject();
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::Reject(payload),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                Ok(())
            }
            PeerMessage::Disconnect => {
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message,
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                Err(PeerError::DisconnectCommand)
            }
        }
    }

    async fn main_thread_request<W>(
        &mut self,
        request: MainThreadMessage,
        writer: &mut W,
        message_generator: &mut Box<dyn MessageGenerator>,
    ) -> Result<(), PeerError>
    where
        W: AsyncWrite + Send + Unpin,
    {
        match request {
            MainThreadMessage::GetAddr => {
                self.message_counter.sent_addrs();
                let message = message_generator.addr()?;
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::GetAddrV2 => {
                let message = message_generator.addrv2()?;
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::GetHeaders(config) => {
                self.message_counter.sent_header();
                let message = message_generator.headers(config.locators, config.stop_hash)?;
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::GetFilterHeaders(config) => {
                self.message_counter.sent_filter_header();
                let message = message_generator.cf_headers(config)?;
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::GetFilters(config) => {
                self.message_counter.sent_filters();
                let message = message_generator.filters(config)?;
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::GetBlock(message) => {
                self.message_counter.sent_block();
                let message = message_generator.block(message)?;
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::BroadcastTx(transaction) => {
                self.message_counter.sent_tx();
                let message = message_generator.transaction(transaction)?;
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::Verack => {
                let message = message_generator.verack()?;
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::Disconnect => return Err(PeerError::DisconnectCommand),
        }
        Ok(())
    }

    async fn write_bytes<W>(&mut self, writer: &mut W, message: Vec<u8>) -> Result<(), PeerError>
    where
        W: AsyncWrite + Send + Unpin,
    {
        writer
            .write_all(&message)
            .await
            .map_err(|_| PeerError::BufferWrite)?;
        writer.flush().await.map_err(|_| PeerError::BufferWrite)?;
        Ok(())
    }

    async fn try_handshake<W, R>(
        &mut self,
        writer: &mut W,
        reader: &mut R,
    ) -> Result<PacketHandler, PeerError>
    where
        W: AsyncWrite + Send + Unpin,
        R: AsyncRead + Send + Unpin,
    {
        self.dialog
            .send_dialog("Initiating a handshake for encrypted messaging".into())
            .await;
        let mut public_key = [0; 64];
        let mut handshake = Handshake::new(self.network, Role::Initiator, None, &mut public_key)
            .map_err(|_| PeerError::HandshakeFailed)?;
        self.write_bytes(writer, public_key.to_vec()).await?;
        let mut remote_public_key = [0u8; 64];
        let _ = reader
            .read_exact(&mut remote_public_key)
            .await
            .map_err(|_| PeerError::Reader)?;
        let mut local_garbage_terminator_message = [0u8; 36];
        handshake
            .complete_materials(remote_public_key, &mut local_garbage_terminator_message)
            .map_err(|_| PeerError::HandshakeFailed)?;
        self.write_bytes(writer, local_garbage_terminator_message.to_vec())
            .await?;
        let mut max_response = MAX_RESPONSE;
        let size = reader
            .read(&mut max_response)
            .await
            .map_err(|_| PeerError::Reader)?;
        let response = &mut max_response[..size];
        handshake
            .authenticate_garbage_and_version(response)
            .map_err(|_| PeerError::HandshakeFailed)?;
        let packet_handler = handshake
            .finalize()
            .map_err(|_| PeerError::HandshakeFailed)?;
        self.dialog
            .send_dialog("Established an encrypted connection".into())
            .await;
        Ok(packet_handler)
    }
}
