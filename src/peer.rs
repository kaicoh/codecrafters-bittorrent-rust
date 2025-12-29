use crate::{BitTorrentError, Result, bencode::Bencode, util::Bytes20};

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

// 4 bytes for IP, 2 bytes for port
const PEER_SIZE: usize = 6;

#[derive(Debug, Clone)]
pub struct Peer(SocketAddrV4);

impl Peer {
    pub fn connect(&self) -> Result<TcpStream> {
        let stream = TcpStream::connect(self.0)?;
        Ok(stream)
    }

    pub fn handshake(&self, info_hash: Bytes20, peer_id: Bytes20) -> Result<Handshake> {
        let mut stream = self.connect()?;

        let msg = Handshake::new(info_hash, peer_id);
        stream.write_all(msg.as_ref())?;

        let mut resp = Handshake::default();
        stream.read_exact(resp.as_mut())?;

        Ok(resp)
    }
}

impl FromStr for Peer {
    type Err = BitTorrentError;

    fn from_str(s: &str) -> Result<Self> {
        let socket_addr: SocketAddrV4 = s.parse()?;
        Ok(Peer(socket_addr))
    }
}

impl TryFrom<&Bencode> for Vec<Peer> {
    type Error = BitTorrentError;

    fn try_from(value: &Bencode) -> Result<Self> {
        let peers = value
            .as_str()?
            .chunks(PEER_SIZE)
            .filter_map(|chunk| {
                if chunk.len() == PEER_SIZE {
                    let mut bytes = [0u8; PEER_SIZE];
                    bytes.copy_from_slice(chunk);
                    let [b0, b1, b2, b3, b4, b5] = bytes;

                    let ip = Ipv4Addr::new(b0, b1, b2, b3);
                    let port = u16::from_be_bytes([b4, b5]);

                    Some(Peer(SocketAddrV4::new(ip, port)))
                } else {
                    None
                }
            })
            .collect();

        Ok(peers)
    }
}

impl fmt::Display for Peer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

const HANDSHAKE_SIZE: usize = 68;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Handshake([u8; HANDSHAKE_SIZE]);

impl Default for Handshake {
    fn default() -> Self {
        Self([0u8; HANDSHAKE_SIZE])
    }
}

impl Handshake {
    pub fn new(info_hash: Bytes20, peer_id: Bytes20) -> Self {
        let mut bytes = [0u8; HANDSHAKE_SIZE];
        bytes[0] = 19; // Length of protocol string
        bytes[1..20].copy_from_slice(b"BitTorrent protocol");
        // Next 8 bytes are reserved (set to zero)
        bytes[28..48].copy_from_slice(info_hash.as_ref());
        bytes[48..68].copy_from_slice(peer_id.as_ref());
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; HANDSHAKE_SIZE] {
        self.deref()
    }

    pub fn info_hash(&self) -> Bytes20 {
        Bytes20::from(&self.0[28..48])
    }

    pub fn peer_id(&self) -> Bytes20 {
        Bytes20::from(&self.0[48..68])
    }
}

impl Deref for Handshake {
    type Target = [u8; HANDSHAKE_SIZE];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Handshake {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<[u8]> for Handshake {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl io::Write for Handshake {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = buf.len().min(HANDSHAKE_SIZE);
        self.0[..len].copy_from_slice(&buf[..len]);
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_peer() {
        let peer = Peer(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080));
        assert_eq!(peer.to_string(), "127.0.0.1:8080");
    }
}
