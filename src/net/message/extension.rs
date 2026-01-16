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

const MESSAGE_TYPE_REQUEST: Bencode = Bencode::Int(0);
const MESSAGE_TYPE_DATA: Bencode = Bencode::Int(1);
const MESSAGE_TYPE_REJECTED: Bencode = Bencode::Int(2);

pub fn is_extension_message(id: u8) -> bool {
    id == MESSAGE_ID_EXTENSION
}

#[derive(Debug, Clone, PartialEq)]
pub enum Extension {
    Handshake(HashMap<String, Bencode>),
    RequestMetadata { ext_id: u8, piece: u32 },
    Metadata { ext_id: u8, piece: u32, data: Bytes },
    Rejected { ext_id: u8, piece: u32 },
}

impl Extension {
    pub fn metadata_ext_id(&self) -> Option<u8> {
        match self {
            Self::Handshake(dict) => {
                if let Some(Bencode::Dict(m)) = dict.get("m")
                    && let Some(Bencode::Int(id)) = m.get("ut_metadata")
                {
                    Some(*id as u8)
                } else {
                    None
                }
            }
            Self::RequestMetadata { ext_id, .. } => Some(*ext_id),
            Self::Metadata { ext_id, .. } => Some(*ext_id),
            Self::Rejected { ext_id, .. } => Some(*ext_id),
        }
    }
}

impl AsBytes for Extension {
    fn as_bytes(&self) -> Result<Bytes> {
        let mut dict_bytes = Vec::new();
        let mut serializer = BencodeSerializer::new(&mut dict_bytes);

        match self {
            Self::Handshake(dict) => {
                dict.serialize(&mut serializer)?;
            }
            Self::RequestMetadata { piece, .. } => {
                let mut dict = HashMap::new();
                dict.insert("msg_type".to_string(), MESSAGE_TYPE_REQUEST); // request
                dict.insert("piece".to_string(), Bencode::Int(*piece as i64));
                dict.serialize(&mut serializer)?;
            }
            Self::Metadata { piece, data, .. } => {
                let mut dict = HashMap::new();
                dict.insert("msg_type".to_string(), MESSAGE_TYPE_DATA); // data
                dict.insert("piece".to_string(), Bencode::Int(*piece as i64));
                dict.insert("total_size".to_string(), Bencode::Int(data.len() as i64));

                dict.serialize(&mut serializer)?;
                dict_bytes.extend_from_slice(data);
            }
            Self::Rejected { piece, .. } => {
                let mut dict = HashMap::new();
                dict.insert("msg_type".to_string(), MESSAGE_TYPE_REJECTED); // rejected
                dict.insert("piece".to_string(), Bencode::Int(*piece as i64));
                dict.serialize(&mut serializer)?;
            }
        };

        // 1 for message ID
        // 1 for extended message ID
        let length = (dict_bytes.len() + 2) as u32;

        let ext_id = match self {
            Self::Handshake(_) => MESSAGE_ID_EXTENSION_HANDSHAKE,
            Self::RequestMetadata { ext_id, .. } => *ext_id,
            Self::Metadata { ext_id, .. } => *ext_id,
            Self::Rejected { ext_id, .. } => *ext_id,
        };

        Ok(length
            .to_be_bytes()
            .into_iter()
            .chain([MESSAGE_ID_EXTENSION, ext_id]) // message ID and extended message ID
            .chain(dict_bytes)
            .collect())
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
            _ => {
                let data_bytes = &bytes[2..];
                let mut deserializer = BencodeDeserializer::new(data_bytes);
                let dict: HashMap<String, Bencode> = Deserialize::deserialize(&mut deserializer)?;

                let msg_type = dict
                    .get("msg_type")
                    .ok_or_else(|| BitTorrentError::DeserdeError("Missing msg_type".to_string()))?;

                let piece = get_int(&dict, "piece")? as u32;

                match *msg_type {
                    MESSAGE_TYPE_REQUEST => Ok(Extension::RequestMetadata { ext_id, piece }),
                    MESSAGE_TYPE_DATA => {
                        let total_size = get_int(&dict, "total_size")? as usize;
                        let bytes = deserializer.read_exact(total_size)?;

                        Ok(Extension::Metadata {
                            ext_id,
                            piece,
                            data: Bytes::from(bytes),
                        })
                    }
                    MESSAGE_TYPE_REJECTED => Ok(Extension::Rejected { ext_id, piece }),
                    _ => Err(BitTorrentError::DeserdeError(
                        "Unknown msg_type".to_string(),
                    )),
                }
            }
        }
    }
}

pub fn handshake() -> Extension {
    let mut dict = HashMap::new();
    dict.insert(
        "m".to_string(),
        Bencode::Dict({
            let mut ext_map = HashMap::new();
            ext_map.insert("ut_metadata".to_string(), Bencode::Int(1));
            ext_map
        }),
    );
    Extension::Handshake(dict)
}

fn get_int(dict: &HashMap<String, Bencode>, key: &str) -> Result<i64> {
    if let Some(Bencode::Int(v)) = dict.get(key) {
        Ok(*v)
    } else {
        Err(BitTorrentError::DeserdeError(format!(
            "Missing or invalid {key}",
        )))
    }
}
