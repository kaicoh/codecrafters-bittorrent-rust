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

pub mod extension;
pub mod peer;

use crate::{BitTorrentError, Result};

use bytes::{Buf, Bytes, BytesMut};
use tokio_util::codec::Decoder;

pub use extension::Extension;
pub use peer::PeerMessage;

const LENGTH_SIZE: usize = 4;

pub trait AsBytes {
    fn as_bytes(&self) -> Result<Bytes>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    KeepAlive,
    PeerMessage(PeerMessage),
    Extension(Extension),
}

impl Message {
    pub fn is_keep_alive(&self) -> bool {
        matches!(self, Self::KeepAlive)
    }

    pub fn is_peer_message(&self) -> bool {
        matches!(self, Self::PeerMessage(_))
    }

    pub fn is_extension(&self) -> bool {
        matches!(self, Self::Extension(_))
    }

    pub fn as_peer_message(&self) -> Option<&PeerMessage> {
        if let Self::PeerMessage(msg) = self {
            Some(msg)
        } else {
            None
        }
    }

    pub fn as_extension(&self) -> Option<&Extension> {
        if let Self::Extension(ext) = self {
            Some(ext)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct MessageDecoder;

impl Decoder for MessageDecoder {
    type Item = Message;
    type Error = BitTorrentError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        if src.len() < LENGTH_SIZE {
            return Ok(None);
        }

        let mut length_bytes = [0u8; LENGTH_SIZE];
        length_bytes.copy_from_slice(&src[..LENGTH_SIZE]);
        let length = u32::from_be_bytes(length_bytes) as usize;

        if length == 0 {
            src.advance(LENGTH_SIZE);
            return Ok(Some(Message::KeepAlive));
        }

        if src.len() < LENGTH_SIZE + length {
            return Ok(None);
        }

        src.advance(LENGTH_SIZE);

        let msg_bytes = src.split_to(length);
        let msg_id = msg_bytes[0];

        if peer::is_peer_message(msg_id) {
            let msg = PeerMessage::try_from(msg_bytes.as_ref())?;
            return Ok(Some(Message::PeerMessage(msg)));
        }

        if extension::is_extension_message(msg_id) {
            let msg = Extension::try_from(msg_bytes.as_ref())?;
            return Ok(Some(Message::Extension(msg)));
        }

        bail!("Unknown message ID: {msg_id}");
    }
}

impl AsBytes for Message {
    fn as_bytes(&self) -> Result<Bytes> {
        match self {
            Self::KeepAlive => Ok(Bytes::from_static(b"\x00\x00\x00\x00")),
            Self::PeerMessage(msg) => msg.as_bytes(),
            Self::Extension(ext) => ext.as_bytes(),
        }
    }
}
