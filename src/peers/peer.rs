use std::net::IpAddr;

use bitcoin::{p2p::ServiceFlags, BlockHash, Network};
use log::info;
use thiserror::Error;
use tokio::{
    io::AsyncWriteExt,
    net::{tcp::OwnedWriteHalf, TcpStream},
    select,
    sync::mpsc::{self, Receiver, Sender},
};

use crate::{
    node::channel_messages::{MainThreadMessage, PeerMessage, PeerThreadMessage},
    p2p::outbound_messages::V1OutboundMessage,
};

use super::reader::Reader;

pub(crate) struct Peer {
    nonce: u32,
    time: Option<i32>,
    height: Option<u32>,
    best_hash: Option<BlockHash>,
    ip_addr: IpAddr,
    port: u16,
    last_message: Option<u64>,
    main_thread_sender: Sender<PeerThreadMessage>,
    main_thread_recv: Receiver<MainThreadMessage>,
    network: Network,
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
            Network::Bitcoin => 8332,
            Network::Testnet => panic!("unimplemented"),
            Network::Signet => 38332,
            Network::Regtest => panic!("unimplemented"),
            _ => unreachable!(),
        };

        Self {
            nonce,
            time: None,
            height: None,
            best_hash: None,
            ip_addr,
            port: port.unwrap_or(default_port),
            last_message: None,
            main_thread_sender,
            main_thread_recv,
            network,
        }
    }

    pub async fn connect(&mut self) -> Result<(), PeerError> {
        println!("Trying TCP connection");
        let mut stream = TcpStream::connect((self.ip_addr, self.port)).await.unwrap();
        let outbound_messages = V1OutboundMessage::new(self.network);
        println!("Writing version message to remote");
        let version_message = outbound_messages.new_version_message(None);
        stream.write_all(&version_message).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let (tx, mut rx) = mpsc::channel(32);
        let mut peer_reader = Reader::new(reader, tx, self.network);
        let _ = tokio::spawn(async move { peer_reader.read_from_remote().await });
        loop {
            select! {
                peer_message = rx.recv() => {
                    match peer_message {
                        Some(message) => {
                            self.handle_peer_message(message, &mut writer, &outbound_messages).await
                        },
                        None => continue,
                    }
                }
                node_message = self.main_thread_recv.recv() => {
                    match node_message {
                        Some(message) => {
                            match message {
                                MainThreadMessage::GetAddr => {
                                    writer.write_all(&outbound_messages.new_get_addr()).await.unwrap()
                                },
                                _ => ()
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
    ) {
        match message {
            PeerMessage::Version(version) => {
                info!("Sending Verack");
                if version.service_flags.has(ServiceFlags::COMPACT_FILTERS) {
                    writer
                        .write_all(&message_generator.new_verack())
                        .await
                        .unwrap();
                    self.main_thread_sender
                        .send(PeerThreadMessage {
                            nonce: self.nonce,
                            message: PeerMessage::Version(version),
                        })
                        .await
                        .unwrap()
                } else {
                    self.main_thread_sender
                        .send(PeerThreadMessage {
                            nonce: self.nonce,
                            message: PeerMessage::Disconnect,
                        })
                        .await
                        .unwrap()
                }
            }
            PeerMessage::Addr(addrs) => self
                .main_thread_sender
                .send(PeerThreadMessage {
                    nonce: self.nonce,
                    message: PeerMessage::Addr(addrs),
                })
                .await
                .unwrap(),
            PeerMessage::Headers(headers) => self
                .main_thread_sender
                .send(PeerThreadMessage {
                    nonce: self.nonce,
                    message: PeerMessage::Headers(headers),
                })
                .await
                .unwrap(),
            PeerMessage::Disconnect => self
                .main_thread_sender
                .send(PeerThreadMessage {
                    nonce: self.nonce,
                    message,
                })
                .await
                .unwrap(),
            PeerMessage::Verack => {}
            PeerMessage::Ping => {}
            PeerMessage::Pong => {}
        }
    }
}

#[derive(Error, Debug)]
pub enum PeerError {
    #[error("reading bytes off the stream failed")]
    ReadBufferError,
}
