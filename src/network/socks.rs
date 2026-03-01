// Partial implementation of RFC 1928, Socks5 protocol
// ref: https://datatracker.ietf.org/doc/html/rfc1928#section-1

use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use hashes::sha3_256;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use super::error::Socks5Error;

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);
const VERSION: u8 = 5;
const NOAUTH: u8 = 0;
const METHODS: u8 = 1;
const CMD_CONNECT: u8 = 1;
const RESPONSE_SUCCESS: u8 = 0;
const RSV: u8 = 0;
const ADDR_TYPE_IPV4: u8 = 1;
const ADDR_TYPE_DOMAIN: u8 = 3;
const ADDR_TYPE_IPV6: u8 = 4;
// Tor constants
const SALT: &[u8] = b".onion checksum";
const TOR_VERSION: u8 = 0x03;
const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

fn pubkey_to_service(ed25519: [u8; 32]) -> String {
    let mut cs_input = Vec::with_capacity(48);
    // SHA3(".onion checksum" + public key + version)
    cs_input.extend_from_slice(SALT);
    cs_input.extend_from_slice(&ed25519);
    cs_input.push(TOR_VERSION);
    let cs = sha3_256::hash(&cs_input).to_byte_array();
    // Onion address = public key + 2 byte checksum + version
    let mut input_buf = [0u8; 35];
    input_buf[..32].copy_from_slice(&ed25519);
    input_buf[32] = cs[0];
    input_buf[33] = cs[1];
    input_buf[34] = TOR_VERSION;
    let mut encoding = base32_encode(&input_buf);
    debug_assert!(encoding.len() == 56);
    encoding.push_str(".onion");
    encoding
}

fn base32_encode(data: &[u8]) -> String {
    let mut result = String::with_capacity((data.len() * 8).div_ceil(5));
    let mut buffer: u64 = 0;
    let mut bits_left: u32 = 0;
    for &byte in data {
        buffer = (buffer << 8) | byte as u64;
        bits_left += 8;
        while bits_left >= 5 {
            bits_left -= 5;
            let index = ((buffer >> bits_left) & 0x1f) as usize;
            result.push(ALPHABET[index] as char);
        }
        // Keep only the unconsumed bits
        buffer &= (1u64 << bits_left) - 1;
    }
    if bits_left > 0 {
        let index = ((buffer << (5 - bits_left)) & 0x1f) as usize;
        result.push(ALPHABET[index] as char);
    }
    result
}

#[derive(Debug)]
pub(crate) enum SocksConnection {
    ClearNet(IpAddr),
    OnionService([u8; 32]),
}

impl SocksConnection {
    fn encode(&self) -> Vec<u8> {
        match self {
            Self::ClearNet(net) => match net {
                IpAddr::V4(ipv4) => ipv4.octets().to_vec(),
                IpAddr::V6(ipv6) => ipv6.octets().to_vec(),
            },
            Self::OnionService(onion) => {
                let service = pubkey_to_service(*onion);
                let enc = service.as_bytes();
                let mut buf = Vec::with_capacity(enc.len() + 1);
                buf.push(enc.len() as u8);
                buf.extend_from_slice(enc);
                buf
            }
        }
    }

    fn type_byte(&self) -> u8 {
        match self {
            Self::ClearNet(net) => match net {
                IpAddr::V4(_) => ADDR_TYPE_IPV4,
                IpAddr::V6(_) => ADDR_TYPE_IPV6,
            },
            Self::OnionService(_) => ADDR_TYPE_DOMAIN,
        }
    }
}

impl From<IpAddr> for SocksConnection {
    fn from(value: IpAddr) -> Self {
        Self::ClearNet(value)
    }
}

impl From<[u8; 32]> for SocksConnection {
    fn from(value: [u8; 32]) -> Self {
        Self::OnionService(value)
    }
}

pub(crate) async fn create_socks5(
    proxy: SocketAddr,
    addr: SocksConnection,
    port: u16,
) -> Result<TcpStream, Socks5Error> {
    // Connect to the proxy, likely a local Tor daemon.
    let timeout = tokio::time::timeout(CONNECTION_TIMEOUT, TcpStream::connect(proxy))
        .await
        .map_err(|_| Socks5Error::ConnectionTimeout)?;
    // Format the destination IP address and port according to the Socks5 spec
    let dest_ip_bytes = addr.encode();
    let dest_port_bytes = port.to_be_bytes();
    let ip_type_byte = addr.type_byte();
    // Begin the handshake by requesting a connection to the proxy.
    let mut tcp_stream = timeout.map_err(|_| Socks5Error::ConnectionFailed)?;
    tcp_stream.write_all(&[VERSION, METHODS, NOAUTH]).await?;
    // Read the response from the proxy
    let mut buf = [0_u8; 2];
    tcp_stream.read_exact(&mut buf).await?;
    if buf[0] != VERSION {
        return Err(Socks5Error::WrongVersion);
    }
    if buf[1] != NOAUTH {
        return Err(Socks5Error::AuthRequired);
    }
    // Write the request to the proxy to connect to our destination
    tcp_stream
        .write_all(&[VERSION, CMD_CONNECT, RSV, ip_type_byte])
        .await?;
    tcp_stream.write_all(&dest_ip_bytes).await?;
    tcp_stream.write_all(&dest_port_bytes).await?;
    // First 4 bytes of the response: version, success/failure, reserved byte, ip type
    let mut buf = [0_u8; 4];
    tcp_stream.read_exact(&mut buf).await?;
    if buf[0] != VERSION {
        return Err(Socks5Error::WrongVersion);
    }
    if buf[1] != RESPONSE_SUCCESS {
        return Err(Socks5Error::ConnectionFailed);
    }
    // Read off the destination of our request
    match buf[3] {
        ADDR_TYPE_IPV4 => {
            // Read the IPv4 address and additional two bytes for the port
            let mut buf = [0_u8; 6];
            tcp_stream.read_exact(&mut buf).await?;
        }
        ADDR_TYPE_IPV6 => {
            // Read the IPv6 address and additional two bytes for the port
            let mut buf = [0_u8; 18];
            tcp_stream.read_exact(&mut buf).await?;
        }
        ADDR_TYPE_DOMAIN => {
            let mut len = [0_u8; 1];
            tcp_stream.read_exact(&mut len).await?;
            let mut buf = vec![0_u8; u8::from_le_bytes(len) as usize];
            tcp_stream.read_exact(&mut buf).await?;
        }
        _ => return Err(Socks5Error::ConnectionFailed),
    }

    // Proxy handshake is complete, the TCP reader/writer can be returned
    Ok(tcp_stream)
}

#[cfg(test)]
mod tests {
    use super::pubkey_to_service;

    #[test]
    fn public_key_to_service() {
        let hex = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        let hsid: [u8; 32] = hex::decode(hex).unwrap().try_into().unwrap();
        let service = pubkey_to_service(hsid);
        assert_eq!(
            "25njqamcweflpvkl73j4szahhihoc4xt3ktcgjnpaingr5yhkenl5sid.onion",
            service
        );
    }
}
