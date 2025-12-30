use super::message::{MessageBuf, PeerMessage};
use crate::{BitTorrentError, Result, bencode::Bencode, util::Bytes20};

use std::cmp;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use tokio::sync::oneshot;

// 4 bytes for IP, 2 bytes for port
const PEER_SIZE: usize = 6;
// 16KB
const BLOCK_SIZE: usize = 16 * 1024;
const PIPELINE_SIZE: usize = 5;

macro_rules! bail {
    ($msg:expr) => {
        return Err(BitTorrentError::InvalidPeerMessage(format!($msg)))
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

        let conn = PeerConnection {
            peer_id: resp.peer_id(),
            stream,
        };

        Ok(conn)
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
pub(crate) struct Handshake([u8; HANDSHAKE_SIZE]);

impl Default for Handshake {
    fn default() -> Self {
        Self([0u8; HANDSHAKE_SIZE])
    }
}

impl Handshake {
    pub(crate) fn new(info_hash: Bytes20, peer_id: Bytes20) -> Self {
        let mut bytes = [0u8; HANDSHAKE_SIZE];
        bytes[0] = 19; // Length of protocol string
        bytes[1..20].copy_from_slice(b"BitTorrent protocol");
        // Next 8 bytes are reserved (set to zero)
        bytes[28..48].copy_from_slice(info_hash.as_ref());
        bytes[48..68].copy_from_slice(peer_id.as_ref());
        Self(bytes)
    }

    pub(crate) fn peer_id(&self) -> Bytes20 {
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
    pub fn ready(&mut self) -> Result<()> {
        self.wait_for_bitfield()?;
        self.send_interested()?;
        self.wait_for_unchoke()
    }

    pub fn peer_id(&self) -> Bytes20 {
        self.peer_id
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

    pub async fn download_piece(&mut self, index: u32, piece_length: u32) -> Result<Vec<u8>> {
        let mut offset = 0;
        let mut tasks = tokio::task::JoinSet::<Download>::new();
        let mut triggers: Vec<oneshot::Sender<()>> = Vec::new();

        while offset < piece_length {
            let block_size = cmp::min(BLOCK_SIZE as u32, piece_length - offset);

            let (tx, rx) = oneshot::channel::<()>();
            triggers.push(tx);

            let mut stream = self.stream.try_clone()?;

            tasks.spawn(async move {
                rx.await.expect("Failed to receive signal");

                let request_msg = PeerMessage::Request {
                    index,
                    begin: offset,
                    length: block_size,
                };
                send_message(&mut stream, request_msg).expect("Failed to send request message");

                loop {
                    let msg = read_message(&mut stream).expect("Failed to read message");
                    if let PeerMessage::Piece {
                        index: msg_index,
                        begin,
                        block,
                    } = msg
                        && msg_index == index
                        && begin == offset
                    {
                        return Download { index, block };
                    }
                }
            });

            offset += block_size;
        }

        let channel_count = cmp::min(triggers.len(), PIPELINE_SIZE);

        let mut downloads: Vec<Download> = Vec::new();
        let mut trigger_iter = triggers.into_iter();

        for _ in 0..channel_count {
            if let Some(tx) = trigger_iter.next() {
                tx.send(()).map_err(|_| BitTorrentError::OneshotSendError)?;
            }
        }

        while let Some(res) = tasks.join_next().await {
            let download = res?;
            downloads.push(download);

            if let Some(tx) = trigger_iter.next() {
                tx.send(()).map_err(|_| BitTorrentError::OneshotSendError)?;
            }
        }

        downloads.sort();

        Ok(downloads.into_iter().flat_map(|d| d.block).collect())
    }

    fn read_message(&mut self) -> Result<PeerMessage> {
        read_message(&mut self.stream)
    }

    fn send_message(&mut self, msg: PeerMessage) -> Result<()> {
        send_message(&mut self.stream, msg)
    }
}

fn read_message(stream: &mut TcpStream) -> Result<PeerMessage> {
    let mut buf = MessageBuf::new();

    loop {
        let mut temp_buf = [0u8; 4096];
        let n = stream.read(&mut temp_buf)?;

        if n == 0 {
            bail!("Connection closed by peer");
        }

        buf.write_all(&temp_buf[..n])?;

        if let Some(msg_result) = buf.build_if_ready() {
            return msg_result;
        }
    }
}

fn send_message(stream: &mut TcpStream, msg: PeerMessage) -> Result<()> {
    let bytes = msg.into_bytes();
    stream.write_all(&bytes)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub index: u32,
    pub block: Vec<u8>,
}

impl std::cmp::PartialOrd for Download {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for Download {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index.cmp(&other.index)
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
