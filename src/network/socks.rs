// Partial implementation of RFC 1928, Socks5 protocol
// ref: https://datatracker.ietf.org/doc/html/rfc1928#section-1

use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

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
const ADDR_TYPE_IPV6: u8 = 4;

pub(crate) async fn create_socks5(
    proxy: SocketAddr,
    ip_addr: IpAddr,
    port: u16,
) -> Result<TcpStream, Socks5Error> {
    // Connect to the proxy, likely a local Tor daemon.
    let timeout = tokio::time::timeout(CONNECTION_TIMEOUT, TcpStream::connect(proxy))
        .await
        .map_err(|_| Socks5Error::ConnectionTimeout)?;
    // Format the destination IP address and port according to the Socks5 spec
    let dest_ip_bytes = match ip_addr {
        IpAddr::V4(ipv4) => ipv4.octets().to_vec(),
        IpAddr::V6(ipv6) => ipv6.octets().to_vec(),
    };
    let dest_port_bytes = port.to_be_bytes();
    let ip_type_byte = match ip_addr {
        IpAddr::V4(_) => ADDR_TYPE_IPV4,
        IpAddr::V6(_) => ADDR_TYPE_IPV6,
    };
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
        _ => return Err(Socks5Error::ConnectionFailed),
    }

    // Proxy handshake is complete, the TCP reader/writer can be returned
    Ok(tcp_stream)
}
