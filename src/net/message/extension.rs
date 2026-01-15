use crate::{
    BitTorrentError, Result,
    bencode::{Bencode, Deserializer as BencodeDeserializer, Serializer as BencodeSerializer},
};

use super::AsBytes;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MESSAGE_ID_EXTENSION: u8 = 20;
const MESSAGE_ID_EXTENSION_HANDSHAKE: u8 = 0;

#[derive(Debug, Clone, PartialEq)]
pub enum Extension {
    Handshake(HashMap<String, Bencode>),
}

pub fn is_extension_message(id: u8) -> bool {
    id == MESSAGE_ID_EXTENSION
}

impl AsBytes for Extension {
    fn as_bytes(&self) -> Result<Bytes> {
        let bytes = match self {
            Self::Handshake(dict) => {
                let mut dict_bytes = Vec::new();
                let mut serializer = BencodeSerializer::new(&mut dict_bytes);
                dict.serialize(&mut serializer)?;

                // 1 for message ID
                // 1 for extended message ID
                let length = (dict_bytes.len() + 2) as u32;

                length
                    .to_be_bytes()
                    .iter()
                    .chain(&[20u8, 0u8]) // message ID and extended message ID
                    .chain(&dict_bytes)
                    .cloned()
                    .collect()
            }
        };

        Ok(bytes)
    }
}

impl TryFrom<&[u8]> for Extension {
    type Error = BitTorrentError;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() >= 2, "Extension message too short");

        let msg_id = bytes[0];
        ensure!(
            msg_id == MESSAGE_ID_EXTENSION,
            "Invalid extension message ID"
        );

        let ext_id = bytes[1];
        match ext_id {
            MESSAGE_ID_EXTENSION_HANDSHAKE => {
                let dict_bytes = &bytes[2..];
                let mut deserializer = BencodeDeserializer::new(dict_bytes);
                let dict: HashMap<String, Bencode> = Deserialize::deserialize(&mut deserializer)?;

                Ok(Extension::Handshake(dict))
            }
            _ => bail!("Unknown extension message ID: {ext_id}"),
        }
    }
}
