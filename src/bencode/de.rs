use super::Bencode;
use serde::de;

pub(crate) struct ByteVisitor;

impl<'de> de::Visitor<'de> for ByteVisitor {
    type Value = Bencode;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a byte array")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let val = Bencode::parse(v)
            .map_err(|e| de::Error::custom(format!("Bencode parsing error: {e}")))?;
        Ok(val)
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_bytes(&v)
    }
}
