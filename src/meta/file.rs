use crate::{
    BitTorrentError,
    bencode::{ByteSeqVisitor, Deserializer, Serializer},
    util::{Bytes20, HASH_SIZE},
};

use serde::{Deserialize, Serialize, de, ser};
use sha1::{Digest, Sha1};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Hashes(Vec<Bytes20>);

impl AsRef<[Bytes20]> for Hashes {
    fn as_ref(&self) -> &[Bytes20] {
        &self.0
    }
}

impl ser::Serialize for Hashes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let len = self.0.iter().map(|i| i.len()).sum();
        let mut bytes = Vec::with_capacity(len);
        for hash in self.as_ref() {
            bytes.extend_from_slice(hash.as_ref());
        }
        serializer.serialize_bytes(&bytes)
    }
}

impl<'de> de::Deserialize<'de> for Hashes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let visitor = ByteSeqVisitor::new(HASH_SIZE);
        let vec = deserializer.deserialize_bytes(visitor)?;
        Ok(Hashes(vec))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Info {
    #[serde(rename = "piece length")]
    pub piece_length: u32,
    pub pieces: Hashes,
    pub name: String,
    pub length: u64,
}

impl Info {
    pub fn piece_hashes(&self) -> &[Bytes20] {
        self.pieces.as_ref()
    }

    pub fn num_pieces(&self) -> usize {
        self.pieces.as_ref().len()
    }

    pub fn match_hash(&self, index: usize, hash: &Bytes20) -> bool {
        self.piece_hashes().get(index).is_some_and(|h| h == hash)
    }

    pub fn hash(&self) -> Result<Bytes20, BitTorrentError> {
        let mut bytes = Vec::new();
        self.serialize(&mut Serializer::new(&mut bytes))?;
        let digest = Sha1::digest(&bytes);
        Ok(Bytes20::from(digest.as_ref()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub announce: String,
    pub info: Info,
}

impl Meta {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, BitTorrentError> {
        let f = fs::File::open(path)?;
        let mut de = Deserializer::new(&f);
        let meta = Meta::deserialize(&mut de)?;
        Ok(meta)
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
            pieces: Hashes(vec![
                Bytes20::from(&hash("hello")[..]),
                Bytes20::from(&hash("world")[..]),
            ]),
            name: "test_file.txt".to_string(),
            length: 32768,
        };

        let mut bytes = Vec::new();
        info.serialize(&mut Serializer::new(&mut bytes)).unwrap();

        let expected = b"d6:lengthi32768e4:name13:test_file.txt12:piece lengthi16384e6:pieces"
            .iter()
            .chain(b"40:")
            .chain(&hash("hello"))
            .chain(&hash("world"))
            .chain(b"e")
            .cloned()
            .collect::<Vec<u8>>();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_info_deserialization() {
        let data = b"d6:lengthi32768e4:name13:test_file.txt12:piece lengthi16384e6:pieces40:"
            .iter()
            .chain(&hash("hello"))
            .chain(&hash("world"))
            .chain(b"e")
            .cloned()
            .collect::<Vec<u8>>();
        let mut de = Deserializer::new(&data[..]);
        let info = Info::deserialize(&mut de).unwrap();
        let expected = Info {
            piece_length: 16384,
            pieces: Hashes(vec![
                Bytes20::from(&hash("hello")[..]),
                Bytes20::from(&hash("world")[..]),
            ]),
            name: "test_file.txt".to_string(),
            length: 32768,
        };

        assert_eq!(info, expected);
    }

    fn hash(v: &str) -> Vec<u8> {
        let mut hasher = Sha1::new();
        hasher.update(v.as_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; HASH_SIZE];
        hash.copy_from_slice(&result);
        hash.to_vec()
    }
}
