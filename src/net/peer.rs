use crate::{BitTorrentError, Result, util::Bytes20};

use super::PeerMessage;

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use tokio::sync::mpsc::{self, Receiver, Sender};

const LENGTH_SIZE: usize = 4;
const PEER_BYTE_SIZE: usize = 6;
const HANDSHAKE_SIZE: usize = 68;

macro_rules! tri {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                eprintln!("Error from spawned thread: {e}");
                break;
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peer(SocketAddrV4);

impl Peer {
    pub fn connect(
        &self,
        info_hash: Bytes20,
        peer_id: Bytes20,
    ) -> Result<(Sender<PeerMessage>, Receiver<PeerMessage>)> {
        let mut stream = TcpStream::connect(self.0)?;

        let msg = Handshake::new(info_hash, peer_id);
        stream.write_all(msg.as_bytes())?;

        let mut resp = Handshake::default();
        stream.read_exact(resp.as_mut())?;

        let rx = start_reading(stream.try_clone()?);
        let tx = start_writing(stream);

        Ok((tx, rx))
    }
}

fn start_reading(stream: TcpStream) -> Receiver<PeerMessage> {
    let (tx, rx) = mpsc::channel::<PeerMessage>(100);
    let mut reader = BufReader::new(stream);

    tokio::spawn(async move {
        loop {
            let buf = tri!(reader.fill_buf());

            if buf.len() < LENGTH_SIZE {
                continue;
            }

            let length = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;

            if buf.len() < length + LENGTH_SIZE {
                continue;
            }

            let mut msg_buf = Vec::with_capacity(length + LENGTH_SIZE);

            let _ = tri!(reader.read_exact(&mut msg_buf[..]));

            let msg = tri!(PeerMessage::try_from(&msg_buf[LENGTH_SIZE..]));

            if let Err(_) = tx.send(msg).await {
                println!("Reader: receiver dropped, stopping reading");
                break;
            }
        }
    });

    rx
}

fn start_writing(mut stream: TcpStream) -> Sender<PeerMessage> {
    let (tx, mut rx) = mpsc::channel::<PeerMessage>(100);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let bytes = msg.into_bytes();
            let _ = tri!(stream.write_all(&bytes));
        }
    });

    tx
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

impl io::Write for Handshake {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() != HANDSHAKE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Invalid length for Handshake: expected {}, got {}",
                    HANDSHAKE_SIZE,
                    buf.len()
                ),
            ));
        }
        self.0.copy_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct PeerStream {
    peer_id: Bytes20,
    stream: TcpStream,
}
