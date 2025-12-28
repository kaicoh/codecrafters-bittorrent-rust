use super::Result;
use serde::ser;

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
    pub fn url_encoded(&self) -> Result<String> {
        let encoded = serde_urlencoded::to_string(self.0)?;
        Ok(encoded)
    }

    pub fn hex_encoded(&self) -> String {
        hex::encode(self.0)
    }
}

impl ser::Serialize for Hash20 {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[test]
    fn test_url_encoded() {
        #[derive(Debug, Serialize)]
        struct TestStruct {
            hash: Hash20,
            chees: String,
        }
        let val = TestStruct {
            hash: Hash20(*b"abcdefghijklmnopqrst"),
            chees: "comté".to_string(),
        };
        let encoded = serde_urlencoded::to_string(val).unwrap();
        assert_eq!(encoded, "hash=abcdefghijklmnopqrst&chees=comt%C3%A9");
    }
}
