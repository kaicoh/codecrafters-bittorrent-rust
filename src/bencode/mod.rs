mod de;
mod ser;

pub(crate) use de::ByteSeqVisitor;
pub use de::Deserializer;
pub use ser::Serializer;

use crate::{BitTorrentError, Result};

use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

macro_rules! bail {
    ($err:expr) => {
        return Err(BitTorrentError::BencodeError($err))
    };
}

#[derive(Debug, Clone, PartialEq)]
pub enum Bencode {
    Str(Vec<u8>),
    Int(i64),
    List(Vec<Bencode>),
    Dict(HashMap<String, Bencode>),
}

impl fmt::Display for Bencode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Str(v) => write!(f, "\"{}\"", String::from_utf8_lossy(v)),
            Self::Int(v) => write!(f, "{v}"),
            Self::List(vals) => write!(
                f,
                "[{}]",
                vals.iter()
                    .map(|v| format!("{v}"))
                    .collect::<Vec<String>>()
                    .join(",")
            ),
            Self::Dict(v) => {
                let mut items: Vec<String> = Vec::new();
                let mut sorted_keys: Vec<&String> = v.keys().collect();
                sorted_keys.sort();

                for key in sorted_keys {
                    let value = &v[key];
                    items.push(format!("\"{}\":{}", key, value));
                }

                write!(f, "{{{}}}", items.join(","))
            }
        }
    }
}

impl From<i64> for Bencode {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<String> for Bencode {
    fn from(value: String) -> Self {
        Self::Str(value.into_bytes())
    }
}

impl<'a> From<&'a str> for Bencode {
    fn from(value: &'a str) -> Self {
        Bencode::Str(value.as_bytes().to_vec())
    }
}

impl Bencode {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut deserializer = Deserializer::new(bytes);
        Bencode::deserialize(&mut deserializer)
    }

    pub fn as_str(&self) -> Result<&[u8]> {
        match self {
            Bencode::Str(v) => Ok(v),
            _ => bail!("expected Bencode::Str"),
        }
    }
}
