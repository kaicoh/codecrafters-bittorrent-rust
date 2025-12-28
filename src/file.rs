use crate::{Result, bencode::Bencode};

use serde::Serialize;
use std::io::Read;

const HASH_SIZE: usize = 20;

#[derive(Debug, Clone, Serialize)]
pub struct Info {
    #[serde(rename = "piece length")]
    pub piece_length: u64,
    pub pieces: Vec<Bencode>,
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
            .map(|chunk| Bencode::Str(chunk.to_vec()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bencode::Serializer;
    use sha1::{Digest, Sha1};

    #[test]
    fn test_info_serialization() {
        let info = Info {
            piece_length: 16384,
            pieces: vec![
                Bencode::Str(hash("hello").to_vec()),
                Bencode::Str(hash("world").to_vec()),
            ],
            name: "test_file.txt".to_string(),
            length: 32768,
        };

        let mut bytes = Vec::new();
        info.serialize(&mut Serializer::new(&mut bytes)).unwrap();

        let expected = b"d6:lengthi32768e4:name13:test_file.txt12:piece lengthi16384e6:piecesl"
            .iter()
            .chain(b"20:")
            .chain(&hash("hello"))
            .chain(b"20:")
            .chain(&hash("world"))
            .chain(b"ee")
            .cloned()
            .collect::<Vec<u8>>();
        assert_eq!(bytes, expected);
    }

    fn hash(v: &str) -> [u8; HASH_SIZE] {
        let mut hasher = Sha1::new();
        hasher.update(v.as_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; HASH_SIZE];
        hash.copy_from_slice(&result);
        hash
    }
}
