pub const HASH_SIZE: usize = 20;

#[derive(Debug, Clone, Copy, Default)]
pub struct Hash20([u8; HASH_SIZE]);

impl From<&[u8]> for Hash20 {
    fn from(slice: &[u8]) -> Self {
        let mut array = [0u8; HASH_SIZE];
        array.copy_from_slice(&slice[0..HASH_SIZE]);
        Hash20(array)
    }
}

impl Hash20 {
    pub fn hex_encoded(&self) -> String {
        hex::encode(self.0)
    }
}

impl AsRef<[u8]> for Hash20 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
