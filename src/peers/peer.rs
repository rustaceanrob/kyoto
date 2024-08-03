extern crate tokio;
use std::{net::IpAddr, time::Duration};

use bitcoin::Network;
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    select,
    sync::mpsc::{self, Receiver, Sender},
};

use crate::{
    node::{
        channel_messages::{MainThreadMessage, PeerMessage, PeerThreadMessage},
        dialog::Dialog,
        messages::Warning,
    },
    peers::outbound_messages::V1OutboundMessage,
};

use super::{
    counter::{MessageCounter, MessageTimer},
    error::PeerError,
    reader::Reader,
    traits::MessageGenerator,
};

const CONNECTION_TIMEOUT: u64 = 2;

pub(crate) struct Peer {
    nonce: u32,
    ip_addr: IpAddr,
    port: u16,
    main_thread_sender: Sender<PeerThreadMessage>,
    main_thread_recv: Receiver<MainThreadMessage>,
    network: Network,
    message_counter: MessageCounter,
    message_timer: MessageTimer,
    dialog: Dialog,
}

impl Peer {
    pub fn new(
        nonce: u32,
        ip_addr: IpAddr,
        port: u16,
        network: Network,
        main_thread_sender: Sender<PeerThreadMessage>,
        main_thread_recv: Receiver<MainThreadMessage>,
        dialog: Dialog,
    ) -> Self {
        let message_counter = MessageCounter::new();
        let message_timer = MessageTimer::new();
        Self {
            nonce,
            ip_addr,
            port,
            main_thread_sender,
            main_thread_recv,
            network,
            message_counter,
            message_timer,
            dialog,
        }
    }

    pub async fn connect(&mut self) -> Result<(), PeerError> {
        let timeout = tokio::time::timeout(
            Duration::from_secs(CONNECTION_TIMEOUT),
            TcpStream::connect((self.ip_addr, self.port)),
        )
        .await
        .map_err(|_| PeerError::TcpConnectionFailed)?;
        // Replace with generalization
        let mut stream: TcpStream;
        if let Ok(tcp) = timeout {
            stream = tcp;
        } else {
            self.dialog.send_warning(Warning::PeerTimedOut).await;
            let _ = self
                .main_thread_sender
                .send(PeerThreadMessage {
                    nonce: self.nonce,
                    message: PeerMessage::Disconnect,
                })
                .await;
            return Err(PeerError::TcpConnectionFailed);
        }
        let mut outbound_messages = V1OutboundMessage::new(self.network);
        let message = outbound_messages.version_message(None);
        self.write_bytes(&mut stream, message).await?;
        self.message_timer.track();
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
                self.dialog.send_warning(Warning::UnsolicitedMessage).await;
                return Ok(());
            }
            if self.message_timer.unresponsive() {
                self.dialog.send_warning(Warning::PeerTimedOut).await;
                return Ok(());
            }
            select! {
                // The peer sent us a message
                peer_message = tokio::time::timeout(Duration::from_secs(CONNECTION_TIMEOUT), rx.recv()) => {
                    if let Ok(peer_message) = peer_message {
                        match peer_message {
                            Some(message) => {
                                match self.handle_peer_message(message, &mut writer, &mut outbound_messages).await {
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
                            match self.main_thread_request(message, &mut writer, &mut outbound_messages).await {
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

    async fn handle_peer_message<M, W>(
        &mut self,
        message: PeerMessage,
        writer: &mut W,
        message_generator: &mut M,
    ) -> Result<(), PeerError>
    where
        M: MessageGenerator,
        W: AsyncWrite + Send + Sync + Unpin,
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
                // FIXME: Write this after confirming the peer has CBF.
                writer
                    .write_all(&message_generator.verack())
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
                writer.flush().await.map_err(|_| PeerError::BufferWrite)?;
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
                self.message_timer.untrack();
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
                self.message_timer.untrack();
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
                self.message_timer.untrack();
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
                self.message_timer.untrack();
                Ok(())
            }
            PeerMessage::Ping(nonce) => {
                let message = message_generator.pong(nonce);
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

    async fn main_thread_request<M, W>(
        &mut self,
        request: MainThreadMessage,
        writer: &mut W,
        message_generator: &mut M,
    ) -> Result<(), PeerError>
    where
        M: MessageGenerator,
        W: AsyncWrite + Send + Sync + Unpin,
    {
        match request {
            MainThreadMessage::GetAddr => {
                self.message_counter.sent_addrs();
                let message = message_generator.addr();
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::GetHeaders(config) => {
                self.message_timer.track();
                let message = message_generator.headers(config.locators, config.stop_hash);
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::GetFilterHeaders(config) => {
                self.message_counter.sent_filter_header();
                self.message_timer.track();
                let message = message_generator.cf_headers(config);
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::GetFilters(config) => {
                self.message_counter.sent_filters();
                let message = message_generator.filters(config);
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::GetBlock(message) => {
                self.message_counter.sent_block();
                self.message_timer.track();
                let message = message_generator.block(message);
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::BroadcastTx(transaction) => {
                self.message_counter.sent_tx();
                let message = message_generator.transaction(transaction);
                self.write_bytes(writer, message).await?;
            }
            MainThreadMessage::Disconnect => return Err(PeerError::DisconnectCommand),
        }
        Ok(())
    }

    async fn write_bytes<W>(&mut self, writer: &mut W, message: Vec<u8>) -> Result<(), PeerError>
    where
        W: AsyncWrite + Send + Sync + Unpin,
    {
        writer
            .write_all(&message)
            .await
            .map_err(|_| PeerError::BufferWrite)?;
        writer.flush().await.map_err(|_| PeerError::BufferWrite)?;
        Ok(())
    }
}
