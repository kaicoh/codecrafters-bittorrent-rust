use crate::{
    BitTorrentError, Result,
    bencode::Bencode,
    util::{Bytes20, HASH_SIZE},
};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Info {
    #[serde(rename = "piece length")]
    pub piece_length: u32,
    pub pieces: Bencode,
    pub name: String,
    pub length: u64,
}

impl Info {
    pub fn piece_hashes(&self) -> Result<Vec<Bytes20>> {
        let hashes = self
            .pieces
            .as_str()?
            .chunks(HASH_SIZE)
            .map(Bytes20::from)
            .collect();
        Ok(hashes)
    }

    pub fn num_pieces(&self) -> Result<usize> {
        let num_pieces = self.pieces.as_str()?.len() / HASH_SIZE;
        Ok(num_pieces)
    }

    pub fn match_hash(&self, index: usize, hash: &Bytes20) -> Result<bool> {
        let result = self.piece_hashes()?.get(index).is_some_and(|h| h == hash);
        Ok(result)
    }
}

impl TryFrom<&Bencode> for Info {
    type Error = BitTorrentError;

    fn try_from(bencode: &Bencode) -> Result<Self> {
        let dict = bencode.as_dict()?;

        let piece_length = dict.get_int("piece length")? as u32;
        let pieces_bytes = dict.get_bytes("pieces")?.to_vec();
        let name = dict.get_str("name")?.to_string();
        let length = dict.get_int("length")? as u64;

        Ok(Info {
            piece_length,
            pieces: Bencode::Str(pieces_bytes),
            name,
            length,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Meta {
    pub announce: String,
    pub info: Info,
}

impl TryFrom<&Bencode> for Meta {
    type Error = BitTorrentError;

    fn try_from(bencode: &Bencode) -> Result<Self> {
        let dict = bencode.as_dict()?;

        let announce = dict.get_str("announce")?.to_string();
        let info_bencode = dict.get("info")?;
        let info = Info::try_from(info_bencode)?;

        Ok(Self { announce, info })
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
            pieces: Bencode::Str(
                hash("hello")
                    .into_iter()
                    .chain(hash("world"))
                    .collect::<Vec<u8>>(),
            ),
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

    fn hash(v: &str) -> Vec<u8> {
        let mut hasher = Sha1::new();
        hasher.update(v.as_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; HASH_SIZE];
        hash.copy_from_slice(&result);
        hash.to_vec()
    }
}
