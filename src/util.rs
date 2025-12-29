use std::ops::Deref;

pub const HASH_SIZE: usize = 20;

#[derive(Debug, Clone, Copy, Default)]
pub struct Bytes20([u8; HASH_SIZE]);

impl From<&[u8]> for Bytes20 {
    fn from(slice: &[u8]) -> Self {
        let mut array = [0u8; HASH_SIZE];
        array.copy_from_slice(&slice[0..HASH_SIZE]);
        Bytes20(array)
    }
}

impl Bytes20 {
    pub fn new(bytes: [u8; HASH_SIZE]) -> Self {
        Bytes20(bytes)
    }

    pub fn hex_encoded(&self) -> String {
        hex::encode(self.0)
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
