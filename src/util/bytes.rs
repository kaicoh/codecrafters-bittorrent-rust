use crate::BitTorrentError;
use std::ops::Deref;

pub const HASH_SIZE: usize = 20;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Bytes20([u8; HASH_SIZE]);

impl From<&[u8]> for Bytes20 {
    fn from(slice: &[u8]) -> Self {
        let mut array = [0u8; HASH_SIZE];
        array.copy_from_slice(&slice[0..HASH_SIZE]);
        Bytes20(array)
    }
}

impl TryFrom<Vec<u8>> for Bytes20 {
    type Error = BitTorrentError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        if value.len() != HASH_SIZE {
            return Err(BitTorrentError::DeserdeError(format!(
                "Invalid length for Bytes20: expected {}, got {}",
                HASH_SIZE,
                value.len()
            )));
        }
        let mut array = [0u8; HASH_SIZE];
        array.copy_from_slice(&value);
        Ok(Bytes20(array))
    }
}

impl Bytes20 {
    pub fn new(bytes: [u8; HASH_SIZE]) -> Self {
        Bytes20(bytes)
    }

    pub fn hex_encoded(&self) -> String {
        hex::encode(self.0)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl AsRef<[u8]> for Bytes20 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Deref for Bytes20 {
    type Target = [u8; HASH_SIZE];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
