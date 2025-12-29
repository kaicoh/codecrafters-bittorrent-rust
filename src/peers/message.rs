use crate::{BitTorrentError, Result};

use std::io;
use std::ops::{Deref, DerefMut};

macro_rules! bail {
    ($msg:expr) => {
        return Err(BitTorrentError::InvalidPeerMessage(format!($msg)))
    };
    ($msg:expr, $($arg:tt)*) => {
        return Err(BitTorrentError::InvalidPeerMessage(format!($msg, $($arg)*)))
    };
}

macro_rules! ensure {
    ($cond:expr, $msg:expr) => {
        if !$cond {
            bail!($msg);
        }
    };
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MessageBuf(Vec<u8>);

impl MessageBuf {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    pub(crate) fn build_if_ready(&self) -> Option<Result<PeerMessage>> {
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

const MESSAGE_ID_CHOKE: u8 = 0;
const MESSAGE_ID_UNCHOKE: u8 = 1;
const MESSAGE_ID_INTERESTED: u8 = 2;
const MESSAGE_ID_NOT_INTERESTED: u8 = 3;
const MESSAGE_ID_HAVE: u8 = 4;
const MESSAGE_ID_BITFIELD: u8 = 5;
const MESSAGE_ID_REQUEST: u8 = 6;
const MESSAGE_ID_PIECE: u8 = 7;
const MESSAGE_ID_CANCEL: u8 = 8;

#[derive(Debug, Clone, PartialEq)]
pub enum PeerMessage {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
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
    Cancel {
        index: u32,
        begin: u32,
        length: u32,
    },
}

impl PeerMessage {
    pub fn into_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::new();

        match self {
            PeerMessage::Choke => {
                bytes.extend_from_slice(&1u32.to_be_bytes());
                bytes.push(MESSAGE_ID_CHOKE);
            }
            PeerMessage::Unchoke => {
                bytes.extend_from_slice(&1u32.to_be_bytes());
                bytes.push(MESSAGE_ID_UNCHOKE);
            }
            PeerMessage::Interested => {
                bytes.extend_from_slice(&1u32.to_be_bytes());
                bytes.push(MESSAGE_ID_INTERESTED);
            }
            PeerMessage::NotInterested => {
                bytes.extend_from_slice(&1u32.to_be_bytes());
                bytes.push(MESSAGE_ID_NOT_INTERESTED);
            }
            PeerMessage::Have(index) => {
                bytes.extend_from_slice(&5u32.to_be_bytes());
                bytes.push(MESSAGE_ID_HAVE);
                bytes.extend_from_slice(&index.to_be_bytes());
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
            PeerMessage::Cancel {
                index,
                begin,
                length,
            } => {
                bytes.extend_from_slice(&13u32.to_be_bytes());
                bytes.push(MESSAGE_ID_CANCEL);
                bytes.extend_from_slice(&index.to_be_bytes());
                bytes.extend_from_slice(&begin.to_be_bytes());
                bytes.extend_from_slice(&length.to_be_bytes());
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
            MESSAGE_ID_CHOKE => PeerMessage::Choke,
            MESSAGE_ID_UNCHOKE => PeerMessage::Unchoke,
            MESSAGE_ID_INTERESTED => PeerMessage::Interested,
            MESSAGE_ID_NOT_INTERESTED => PeerMessage::NotInterested,
            MESSAGE_ID_HAVE => {
                ensure!(payload.len() == 4, "Invalid Have message payload length");
                let index = u32_from_bytes(payload);
                PeerMessage::Have(index)
            }
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
            MESSAGE_ID_CANCEL => {
                ensure!(payload.len() == 12, "Invalid Cancel message payload length");

                let index = u32_from_bytes(&payload[..4]);
                let begin = u32_from_bytes(&payload[4..8]);
                let length = u32_from_bytes(&payload[8..12]);

                PeerMessage::Cancel {
                    index,
                    begin,
                    length,
                }
            }
            _ => bail!("Unknown message ID: {id}"),
        };

        Ok(msg)
    }
}

fn u32_from_bytes(bytes: &[u8]) -> u32 {
    let mut array = [0u8; 4];
    array.copy_from_slice(&bytes[0..4]);
    u32::from_be_bytes(array)
}
