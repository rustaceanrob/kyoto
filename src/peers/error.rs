use crate::impl_sourceless_error;

#[derive(Debug)]
pub(crate) enum PeerReadError {
    ReadBuffer,
    Deserialization,
    TooManyMessages,
    PeerTimeout,
    MpscChannel,
}

impl core::fmt::Display for PeerReadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PeerReadError::ReadBuffer => write!(f, "reading bytes off the stream failed."),
            PeerReadError::Deserialization => {
                write!(f, "the message could not be properly deserialized.")
            }
            PeerReadError::TooManyMessages => write!(f, "DOS protection."),
            PeerReadError::PeerTimeout => write!(f, "peer timeout."),
            PeerReadError::MpscChannel => write!(f, "sending over the channel failed."),
        }
    }
}

impl_sourceless_error!(PeerReadError);

#[derive(Debug)]
pub enum PeerError {
    TcpConnectionFailed,
    BufferWrite,
    ThreadChannel,
    DisconnectCommand,
    Reader,
    UnreachableSocketAddr,
}

impl core::fmt::Display for PeerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PeerError::TcpConnectionFailed => {
                write!(f, "the peer's TCP port was closed or we could not connect.")
            }
            PeerError::BufferWrite => write!(f, "a message could not be written to the peer."),
            PeerError::ThreadChannel => write!(
                f,
                "experienced an error sending a message over the channel."
            ),
            PeerError::DisconnectCommand => {
                write!(f, "the main thread advised this peer to disconnect.")
            }
            PeerError::Reader => write!(f, "the reading thread encountered an error."),
            PeerError::UnreachableSocketAddr => {
                write!(f, "cannot make use of provided p2p address.")
            }
        }
    }
}

impl_sourceless_error!(PeerError);

#[derive(Debug)]
pub(crate) enum DnsBootstrapError {
    NotEnoughPeersError,
}

impl core::fmt::Display for DnsBootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DnsBootstrapError::NotEnoughPeersError => write!(f, "most dns seeding failed"),
        }
    }
}

impl_sourceless_error!(DnsBootstrapError);

#[derive(Debug)]
pub(crate) enum DNSQueryError {
    MessageID,
    Question,
    ConnectionDenied,
    Udp,
    MalformedHeader,
    UnexpectedEOF,
}

impl core::fmt::Display for DNSQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DNSQueryError::ConnectionDenied => write!(f, "the UDP connection failed."),
            DNSQueryError::MalformedHeader => write!(f, "the DNS response header was too short."),
            DNSQueryError::UnexpectedEOF => {
                write!(f, "the end of the response was reached before we expected.")
            }
            DNSQueryError::Udp => write!(f, "reading or writing from the UDP connection failed."),
            DNSQueryError::MessageID => write!(f, "mismatch of message ID"),
            DNSQueryError::Question => write!(f, "the question of the message does not match"),
        }
    }
}

impl_sourceless_error!(DNSQueryError);
