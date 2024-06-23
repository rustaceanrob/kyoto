extern crate tokio;
use std::{net::IpAddr, time::Duration};

use bitcoin::Network;
use thiserror::Error;
use tokio::{
    io::AsyncWriteExt,
    net::{tcp::OwnedWriteHalf, TcpStream},
    select,
    sync::mpsc::{self, Receiver, Sender},
};

use crate::{
    node::channel_messages::{MainThreadMessage, PeerMessage, PeerThreadMessage},
    peers::outbound_messages::V1OutboundMessage,
};

use super::{counter::MessageCounter, reader::Reader};

const CONNECTION_TIMEOUT: u64 = 3;

pub(crate) struct Peer {
    nonce: u32,
    ip_addr: IpAddr,
    port: u16,
    main_thread_sender: Sender<PeerThreadMessage>,
    main_thread_recv: Receiver<MainThreadMessage>,
    network: Network,
    message_counter: MessageCounter,
}

impl Peer {
    pub fn new(
        nonce: u32,
        ip_addr: IpAddr,
        port: u16,
        network: Network,
        main_thread_sender: Sender<PeerThreadMessage>,
        main_thread_recv: Receiver<MainThreadMessage>,
    ) -> Self {
        let message_counter = MessageCounter::new();
        Self {
            nonce,
            ip_addr,
            port,
            main_thread_sender,
            main_thread_recv,
            network,
            message_counter,
        }
    }

    pub async fn connect(&mut self) -> Result<(), PeerError> {
        let timeout = tokio::time::timeout(
            Duration::from_secs(CONNECTION_TIMEOUT),
            TcpStream::connect((self.ip_addr, self.port)),
        )
        .await
        .map_err(|_| PeerError::TcpConnectionFailed)?;
        let mut stream: TcpStream;
        if let Ok(tcp) = timeout {
            stream = tcp;
        } else {
            let _ = self
                .main_thread_sender
                .send(PeerThreadMessage {
                    nonce: self.nonce,
                    message: PeerMessage::Disconnect,
                })
                .await;
            return Err(PeerError::TcpConnectionFailed);
        }
        let outbound_messages = V1OutboundMessage::new(self.network);
        let version_message = outbound_messages.new_version_message(None);
        stream
            .write_all(&version_message)
            .await
            .map_err(|_| PeerError::BufferWrite)?;
        let (reader, mut writer) = stream.into_split();
        let (tx, mut rx) = mpsc::channel(32);
        let mut peer_reader = Reader::new(reader, tx, self.network);
        let read_handle = tokio::spawn(async move {
            match peer_reader.read_from_remote().await {
                Ok(_) => Ok(()),
                Err(_) => Err(PeerError::Reader),
            }
        });
        loop {
            if read_handle.is_finished() {
                return Ok(());
            }
            if self.message_counter.unsolicited() {
                return Ok(());
            }
            select! {
                // The peer sent us a message
                peer_message = rx.recv() => {
                    match peer_message {
                        Some(message) => {
                            match self.handle_peer_message(message, &mut writer, &outbound_messages).await {
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
                // The main thread sent us a message
                node_message = self.main_thread_recv.recv() => {
                    match node_message {
                        Some(message) => {
                            match self.main_thread_request(message, &mut writer, &outbound_messages).await {
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

    async fn handle_peer_message(
        &mut self,
        message: PeerMessage,
        writer: &mut OwnedWriteHalf,
        message_generator: &V1OutboundMessage,
    ) -> Result<(), PeerError> {
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
                writer
                    .write_all(&message_generator.new_verack())
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
                Ok(())
            }
            PeerMessage::Addr(addrs) => {
                self.message_counter.got_addrs();
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
                writer
                    .write_all(&message_generator.new_pong(nonce))
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
                Ok(())
            }
            PeerMessage::Pong(_) => Ok(()),
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

    async fn main_thread_request(
        &mut self,
        request: MainThreadMessage,
        writer: &mut OwnedWriteHalf,
        message_generator: &V1OutboundMessage,
    ) -> Result<(), PeerError> {
        match request {
            MainThreadMessage::GetAddr => {
                self.message_counter.sent_addrs();
                writer
                    .write_all(&message_generator.new_get_addr())
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
            }
            MainThreadMessage::GetHeaders(config) => {
                self.message_counter.sent_header();
                let message = message_generator.new_get_headers(config.locators, config.stop_hash);
                writer
                    .write_all(&message)
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
            }
            MainThreadMessage::GetFilterHeaders(config) => {
                self.message_counter.sent_filter_header();
                let message = message_generator.new_cf_headers(config);
                writer
                    .write_all(&message)
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
            }
            MainThreadMessage::GetFilters(config) => {
                self.message_counter.sent_filters();
                let message = message_generator.new_filters(config);
                writer
                    .write_all(&message)
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
            }
            MainThreadMessage::GetBlock(message) => {
                self.message_counter.sent_block();
                let message = message_generator.new_block(message);
                writer
                    .write_all(&message)
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
            }
            MainThreadMessage::BroadcastTx(transaction) => {
                let message = message_generator.new_transaction(transaction);
                writer
                    .write_all(&message)
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
            }
            MainThreadMessage::Disconnect => return Err(PeerError::DisconnectCommand),
        }
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum PeerError {
    #[error("the peer's TCP port was closed or we could not connect")]
    TcpConnectionFailed,
    #[error("a message could not be written to the peer")]
    BufferWrite,
    #[error("experienced an error sending a message over the channel")]
    ThreadChannel,
    #[error("the main thread advised this peer to disconnect")]
    DisconnectCommand,
    #[error("the ereading thread encountered an error")]
    Reader,
}
