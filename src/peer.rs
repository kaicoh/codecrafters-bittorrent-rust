use crate::{BitTorrentError, Result, bencode::Bencode, util::Bytes20};

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

// 4 bytes for IP, 2 bytes for port
const PEER_SIZE: usize = 6;

macro_rules! bail {
    ($msg:expr) => {
        return Err(BitTorrentError::InvalidPeerMessage($msg))
    };
}

macro_rules! ensure {
    ($cond:expr, $msg:expr) => {
        if !$cond {
            bail!($msg);
        }
    };
}

#[derive(Debug, Clone)]
pub struct Peer(SocketAddrV4);

impl Peer {
    pub fn connect(&self, info_hash: Bytes20, peer_id: Bytes20) -> Result<PeerConnection> {
        let mut stream = TcpStream::connect(self.0)?;

        let msg = Handshake::new(info_hash, peer_id);
        stream.write_all(msg.as_ref())?;

        let mut resp = Handshake::default();
        stream.read_exact(resp.as_mut())?;

        Ok(PeerConnection {
            peer_id: resp.peer_id(),
            stream,
        })
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

#[derive(Debug)]
pub struct PeerConnection {
    peer_id: Bytes20,
    stream: TcpStream,
}

impl PeerConnection {
    pub fn peer_id(&self) -> Bytes20 {
        self.peer_id
    }

    pub fn read_message(&mut self) -> Result<PeerMessage> {
        let mut buf = MessageBuf::new();

        loop {
            let mut temp_buf = [0u8; 4096];
            let n = self.stream.read(&mut temp_buf)?;

            if n == 0 {
                bail!("Connection closed by peer");
            }

            buf.write_all(&temp_buf[..n])?;

            if let Some(msg_result) = buf.build_if_ready() {
                return msg_result;
            }
        }
    }

    pub fn send_message(&mut self, msg: PeerMessage) -> Result<()> {
        let bytes = msg.into_bytes();
        self.stream.write_all(&bytes)?;
        Ok(())
    }

    pub fn wait_for_bitfield(&mut self) -> Result<Vec<u8>> {
        loop {
            let msg = self.read_message()?;
            if let PeerMessage::Bitfield(bitfield) = msg {
                return Ok(bitfield);
            }
        }
    }

    pub fn send_interested(&mut self) -> Result<()> {
        let msg = PeerMessage::Interested;
        self.send_message(msg)
    }

    pub fn wait_for_unchoke(&mut self) -> Result<()> {
        loop {
            let msg = self.read_message()?;
            if let PeerMessage::Unchoke = msg {
                return Ok(());
            }
        }
    }

    pub fn download_piece(&mut self, index: u32, piece_length: u32) -> Result<Vec<u8>> {
        let mut downloaded = vec![0u8; piece_length as usize];
        let mut offset = 0;

        while offset < piece_length {
            let block_size = std::cmp::min(BLOCK_SIZE as u32, piece_length - offset);
            let request_msg = PeerMessage::Request {
                index,
                begin: offset,
                length: block_size,
            };
            self.send_message(request_msg)?;

            loop {
                let msg = self.read_message()?;
                if let PeerMessage::Piece {
                    index: msg_index,
                    begin,
                    block,
                } = msg
                    && msg_index == index
                    && begin == offset
                {
                    downloaded[offset as usize..(offset + block_size) as usize]
                        .copy_from_slice(&block);
                    offset += block_size;
                    break;
                }
            }
        }

        Ok(downloaded)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct MessageBuf(Vec<u8>);

impl MessageBuf {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn build_if_ready(&self) -> Option<Result<PeerMessage>> {
        if self.0.len() < 4 {
            return None;
        }

        let mut bytes: [u8; 4] = [0u8; 4];
        bytes.copy_from_slice(&self.0[0..4]);

        let length = u32::from_be_bytes(bytes) as usize;

        if self.0.len() < length + 4 {
            return None;
        }

        let msg = &self.0[4..4 + length];

        Some(PeerMessage::try_from(msg))
    }
}

impl AsRef<[u8]> for MessageBuf {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Deref for MessageBuf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MessageBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl io::Write for MessageBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

const MESSAGE_ID_UNCHOKE: u8 = 1;
const MESSAGE_ID_INTERESTED: u8 = 2;
const MESSAGE_ID_BITFIELD: u8 = 5;
const MESSAGE_ID_REQUEST: u8 = 6;
const MESSAGE_ID_PIECE: u8 = 7;

const BLOCK_SIZE: usize = 16 * 1024; // 16 KB

#[derive(Debug, Clone, PartialEq)]
pub enum PeerMessage {
    Unchoke,
    Interested,
    Bitfield(Vec<u8>),
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        begin: u32,
        block: Vec<u8>,
    },
}

impl PeerMessage {
    pub fn into_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::new();

        match self {
            PeerMessage::Unchoke => {
                bytes.extend_from_slice(&1u32.to_be_bytes());
                bytes.push(MESSAGE_ID_UNCHOKE);
            }
            PeerMessage::Interested => {
                bytes.extend_from_slice(&1u32.to_be_bytes());
                bytes.push(MESSAGE_ID_INTERESTED);
            }
            PeerMessage::Bitfield(bitfield) => {
                let length = 1 + bitfield.len() as u32;
                bytes.extend_from_slice(&length.to_be_bytes());
                bytes.push(MESSAGE_ID_BITFIELD);
                bytes.extend_from_slice(&bitfield);
            }
            PeerMessage::Request {
                index,
                begin,
                length,
            } => {
                bytes.extend_from_slice(&13u32.to_be_bytes());
                bytes.push(MESSAGE_ID_REQUEST);
                bytes.extend_from_slice(&index.to_be_bytes());
                bytes.extend_from_slice(&begin.to_be_bytes());
                bytes.extend_from_slice(&length.to_be_bytes());
            }
            PeerMessage::Piece {
                index,
                begin,
                block,
            } => {
                let length = 1 + 8 + block.len() as u32;
                bytes.extend_from_slice(&length.to_be_bytes());
                bytes.push(MESSAGE_ID_PIECE);
                bytes.extend_from_slice(&index.to_be_bytes());
                bytes.extend_from_slice(&begin.to_be_bytes());
                bytes.extend_from_slice(&block);
            }
        }

        bytes
    }
}

impl TryFrom<&[u8]> for PeerMessage {
    type Error = BitTorrentError;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        ensure!(!bytes.is_empty(), "Message too short");

        let id = bytes[0];
        let payload = &bytes[1..];

        let msg = match id {
            MESSAGE_ID_UNCHOKE => PeerMessage::Unchoke,
            MESSAGE_ID_INTERESTED => PeerMessage::Interested,
            MESSAGE_ID_BITFIELD => PeerMessage::Bitfield(payload.to_vec()),
            MESSAGE_ID_REQUEST => {
                ensure!(
                    payload.len() == 12,
                    "Invalid Request message payload length"
                );

                let index = u32_from_bytes(&payload[..4]);
                let begin = u32_from_bytes(&payload[4..8]);
                let length = u32_from_bytes(&payload[8..12]);

                PeerMessage::Request {
                    index,
                    begin,
                    length,
                }
            }
            MESSAGE_ID_PIECE => {
                ensure!(payload.len() >= 8, "Invalid Piece message payload length");

                let index = u32_from_bytes(&payload[..4]);
                let begin = u32_from_bytes(&payload[4..8]);
                let block = payload[8..].to_vec();

                PeerMessage::Piece {
                    index,
                    begin,
                    block,
                }
            }
            _ => bail!("Unknown message ID"),
        };

        Ok(msg)
    }
}

fn u32_from_bytes(bytes: &[u8]) -> u32 {
    let mut array = [0u8; 4];
    array.copy_from_slice(&bytes[0..4]);
    u32::from_be_bytes(array)
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
