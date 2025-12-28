use crate::{Result, bencode::Bencode};

use serde::Serialize;
use std::io::Read;

const HASH_SIZE: usize = 20;

#[derive(Debug, Clone, Serialize)]
pub struct Info {
    pub piece_length: u64,
    pub pieces: Vec<[u8; HASH_SIZE]>,
    pub name: String,
    pub length: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetaInfo {
    pub announce: String,
    pub info: Info,
}

impl MetaInfo {
    pub fn new<R: Read>(mut reader: R) -> Result<Self> {
        let bencode = Bencode::from_reader(&mut reader)?;
        let dict = bencode.as_dict()?;

        let announce = dict.get_str("announce")?.to_string();

        let info_bencode = dict.get("info")?;
        let info_dict = info_bencode.as_dict()?;

        let piece_length = info_dict.get_int("piece length")? as u64;
        let pieces_bytes = info_dict.get_bytes("pieces")?;
        let name = info_dict.get_str("name")?.to_string();
        let length = info_dict.get_int("length")? as u64;

        let pieces = pieces_bytes
            .chunks(HASH_SIZE)
            .map(|chunk| {
                let mut arr = [0u8; HASH_SIZE];
                arr.copy_from_slice(chunk);
                arr
            })
            .collect();

        let info = Info {
            piece_length,
            pieces,
            name,
            length,
        };

        Ok(MetaInfo { announce, info })
    }
}
