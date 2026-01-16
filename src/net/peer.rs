use crate::{BitTorrentError, Result, util::Bytes20};

use super::message::{AsBytes, Extension, Message, MessageDecoder, PeerMessage, extension};

use std::fmt;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{
    TcpStream,
    tcp::{OwnedReadHalf, OwnedWriteHalf},
};
use tokio_stream::StreamExt;
use tokio_util::codec::FramedRead;

pub const PEER_BYTE_SIZE: usize = 6;
const HANDSHAKE_SIZE: usize = 68;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peer(SocketAddrV4);

impl Peer {
    pub async fn connect(&self, info_hash: Bytes20, peer_id: Bytes20) -> Result<PeerStream> {
        let mut stream = TcpStream::connect(self.0).await?;

        let msg = Handshake::new(info_hash, peer_id);
        stream.write_all(msg.as_bytes()).await?;

        let mut resp = Handshake::default();
        stream.read_exact(resp.as_mut()).await?;

        let peer_id = resp.peer_id();

        Ok(PeerStream::new(peer_id, stream))
    }
}

impl FromStr for Peer {
    type Err = BitTorrentError;

    fn from_str(s: &str) -> Result<Self> {
        let socket_addr: SocketAddrV4 = s.parse()?;
        Ok(Peer(socket_addr))
    }
}

impl TryFrom<Vec<u8>> for Peer {
    type Error = BitTorrentError;

    fn try_from(v: Vec<u8>) -> Result<Self> {
        if v.len() != PEER_BYTE_SIZE {
            return Err(BitTorrentError::DeserdeError(format!(
                "Invalid length for Peer: expected {}, got {}",
                PEER_BYTE_SIZE,
                v.len()
            )));
        }

        let ip = Ipv4Addr::new(v[0], v[1], v[2], v[3]);
        let port = u16::from_be_bytes([v[4], v[5]]);
        let socket_addr = SocketAddrV4::new(ip, port);

        Ok(Peer(socket_addr))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Handshake([u8; HANDSHAKE_SIZE]);

impl Default for Handshake {
    fn default() -> Self {
        Self([0u8; HANDSHAKE_SIZE])
    }
}

impl Handshake {
    fn new(info_hash: Bytes20, peer_id: Bytes20) -> Self {
        let mut bytes = [0u8; HANDSHAKE_SIZE];
        bytes[0] = 19; // Length of protocol string
        bytes[1..20].copy_from_slice(b"BitTorrent protocol");
        bytes[20..28].copy_from_slice(b"\x00\x00\x00\x00\x00\x10\x00\x00");
        bytes[28..48].copy_from_slice(info_hash.as_ref());
        bytes[48..68].copy_from_slice(peer_id.as_ref());
        Self(bytes)
    }

    fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    fn peer_id(&self) -> Bytes20 {
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

impl fmt::Display for Peer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub struct PeerStream {
    peer_id: Bytes20,
    pub(crate) reader: FramedRead<OwnedReadHalf, MessageDecoder>,
    pub(crate) writer: OwnedWriteHalf,
}

impl PeerStream {
    pub fn new(peer_id: Bytes20, stream: TcpStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        let reader = FramedRead::new(read_half, MessageDecoder);

        Self {
            peer_id,
            reader,
            writer: write_half,
        }
    }

    pub fn peer_id(&self) -> Bytes20 {
        self.peer_id
    }

    pub async fn ready(&mut self) -> Result<()> {
        self.wait_bitfield().await?;
        self.send_interested().await?;
        self.wait_unchoke().await?;
        Ok(())
    }

    pub async fn extension_handshake(&mut self) -> Result<Extension> {
        self.wait_bitfield().await?;
        self.send_message(extension::handshake()).await?;
        self.wait_extention().await
    }

    pub async fn send_message<T: AsBytes>(&mut self, msg: T) -> Result<()> {
        let bytes = msg.as_bytes()?;
        self.writer.write_all(&bytes).await?;
        Ok(())
    }

    pub async fn wait_bitfield(&mut self) -> Result<Message> {
        self.wait_message(|msg| msg.as_peer_message().is_some_and(PeerMessage::is_bitfield))
            .await
    }

    pub async fn wait_extention(&mut self) -> Result<Extension> {
        if let Message::Extension(ext) = self.wait_message(Message::is_extension).await? {
            Ok(ext)
        } else {
            unreachable!()
        }
    }

    async fn send_interested(&mut self) -> Result<()> {
        self.send_message(PeerMessage::Interested).await
    }

    async fn wait_unchoke(&mut self) -> Result<Message> {
        self.wait_message(|msg| msg.as_peer_message().is_some_and(PeerMessage::is_unchoke))
            .await
    }

    pub async fn wait_message<P>(&mut self, predicate: P) -> Result<Message>
    where
        P: Fn(&Message) -> bool,
    {
        while let Some(msg) = self.reader.next().await {
            let msg = msg?;
            if predicate(&msg) {
                return Ok(msg);
            }
        }
        Err(BitTorrentError::ConnectionClosed)
    }
}
