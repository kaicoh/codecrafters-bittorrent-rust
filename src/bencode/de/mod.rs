mod deserializer;
mod visitors;

pub use deserializer::Deserializer;
pub(crate) use visitors::ByteSeqVisitor;

use super::Bencode;

use paste::paste;
use serde::de;
use std::collections::HashMap;
use std::fmt;

macro_rules! visit_int {
    ($($ty:ty)*) => {
        paste! {
            $(
                fn [<visit_ $ty>]<E>(self, v: $ty) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(Bencode::Int(v as i64))
                }
            )*
        }
    }
}

impl<'de> de::Deserialize<'de> for Bencode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct BencodeVisitor;

        impl<'de> de::Visitor<'de> for BencodeVisitor {
            type Value = Bencode;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a bencoded value")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Bencode::Str(v.to_vec()))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Bencode::Int(v))
            }

            visit_int! { u8 u16 u32 u64 i8 i16 i32 }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut elements = Vec::new();
                while let Some(elem) = seq.next_element()? {
                    elements.push(elem);
                }
                Ok(Bencode::List(elements))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut entries = HashMap::new();
                while let Some((key, value)) = map.next_entry()? {
                    entries.insert(key, value);
                }
                Ok(Bencode::Dict(entries))
            }
        }

        deserializer.deserialize_any(BencodeVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn test_deserialize_bencode_int() {
        let data = b"i42e";
        let mut de = Deserializer::new(&data[..]);
        let bencode: Bencode = Deserialize::deserialize(&mut de).unwrap();
        assert_eq!(bencode, Bencode::Int(42));
    }
}
