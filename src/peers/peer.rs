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

use super::reader::Reader;

pub(crate) struct Peer {
    nonce: u32,
    ip_addr: IpAddr,
    port: u16,
    main_thread_sender: Sender<PeerThreadMessage>,
    main_thread_recv: Receiver<MainThreadMessage>,
    network: Network,
}

enum State {
    NotConnected,
    Connected,
    Verack,
}

impl Peer {
    pub fn new(
        nonce: u32,
        ip_addr: IpAddr,
        port: Option<u16>,
        network: Network,
        main_thread_sender: Sender<PeerThreadMessage>,
        main_thread_recv: Receiver<MainThreadMessage>,
    ) -> Self {
        let default_port = match network {
            Network::Bitcoin => 8333,
            Network::Testnet => 18333,
            Network::Signet => 38333,
            Network::Regtest => panic!("unimplemented"),
            _ => unreachable!(),
        };

        Self {
            nonce,
            ip_addr,
            port: port.unwrap_or(default_port),
            main_thread_sender,
            main_thread_recv,
            network,
        }
    }

    pub async fn connect(&mut self) -> Result<(), PeerError> {
        println!("Trying TCP connection to {}", self.ip_addr.to_string());
        let timeout = tokio::time::timeout(
            Duration::from_secs(5),
            TcpStream::connect((self.ip_addr, self.port)),
        )
        .await
        .map_err(|_| PeerError::TcpConnectionFailed)?;
        let mut stream: TcpStream;
        if let Ok(tcp) = timeout {
            println!("Socket accepted");
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
        println!("Writing version message to remote");
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
                Ok(_) => return Ok(()),
                Err(e) => {
                    println!("Our peer likely closed the connection: {}", e.to_string());
                    return Err(PeerError::Reader);
                }
            }
        });
        loop {
            if read_handle.is_finished() {
                return Ok(());
            }
            select! {
                // the peer sent us a message
                peer_message = rx.recv() => {
                    match peer_message {
                        Some(message) => {
                            match self.handle_peer_message(message, &mut writer, &outbound_messages).await {
                                Ok(()) => continue,
                                Err(e) => {
                                    match e {
                                        // we were told by the reader thread to disconnect from this peer
                                        PeerError::DisconnectCommand => return Ok(()),
                                        _ => continue,
                                    }
                                },
                            }
                        },
                        None => continue,
                    }
                }
                // the main thread sent us a message
                node_message = self.main_thread_recv.recv() => {
                    match node_message {
                        Some(message) => {
                            match self.main_thread_request(message, &mut writer, &outbound_messages).await {
                                Ok(()) => continue,
                                Err(e) => {
                                    match e {
                                        // we were told by the main thread to disconnect from this peer
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
                println!("[Peer {}]: sent version", self.nonce);
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::Version(version),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                println!("Sending Verack");
                writer
                    .write_all(&message_generator.new_verack())
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
                // println!("Asking for addrs");
                // writer
                //     .write_all(&message_generator.new_get_addr())
                //     .await
                //     .map_err(|_| PeerError::BufferWrite)?;
                // can ask for addresses here depending on if we need them
                return Ok(());
            }
            PeerMessage::Addr(addrs) => {
                println!("[Peer {}]: sent addresses", self.nonce);
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::Addr(addrs),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                return Ok(());
            }
            PeerMessage::Headers(headers) => {
                println!("[Peer {}]: sent headers", self.nonce);
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::Headers(headers),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                return Ok(());
            }
            PeerMessage::FilterHeaders(cf_headers) => {
                println!("[Peer {}]: sent filter headers", self.nonce);
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::FilterHeaders(cf_headers),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                return Ok(());
            }
            PeerMessage::Filter(filter) => {
                println!("[Peer {}]: sent a filter", self.nonce);
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message: PeerMessage::Filter(filter),
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                return Ok(());
            }
            PeerMessage::Disconnect => {
                println!("[Peer {}]: disconnecting", self.nonce);
                self.main_thread_sender
                    .send(PeerThreadMessage {
                        nonce: self.nonce,
                        message,
                    })
                    .await
                    .map_err(|_| PeerError::ThreadChannel)?;
                return Err(PeerError::DisconnectCommand);
            }
            PeerMessage::Verack => Ok(()),
            PeerMessage::Ping(nonce) => {
                println!("[Peer {}]: ping", self.nonce);
                writer
                    .write_all(&message_generator.new_pong(nonce))
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
                Ok(())
            }
            PeerMessage::Pong(_) => Ok(()),
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
                writer
                    .write_all(&message_generator.new_get_addr())
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
            }
            MainThreadMessage::GetHeaders(config) => {
                let message = message_generator.new_get_headers(config.locators, config.stop_hash);
                writer
                    .write_all(&message)
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
            }
            MainThreadMessage::GetFilterHeaders(config) => {
                let message = message_generator.new_cf_headers(config);
                writer
                    .write_all(&message)
                    .await
                    .map_err(|_| PeerError::BufferWrite)?;
            }
            MainThreadMessage::GetFilters(config) => {
                let message = message_generator.new_filters(config);
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

pub(crate) struct PeerConfig {
    find_addrs: FindAddresses,
    cpf_policy: CPFilterPolicy,
}

pub(crate) enum FindAddresses {
    None,
    CPF,
    Any,
}

pub(crate) enum CPFilterPolicy {
    BlockHeadersOnly,
    MustHaveCPFilters,
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
