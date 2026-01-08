use crate::{BitTorrentError, Result};
use paste::paste;
use serde::{de, forward_to_deserialize_any};
use std::io::{BufRead, BufReader, Read};

impl de::Error for BitTorrentError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        BitTorrentError::DeserdeError(msg.to_string())
    }
}

macro_rules! err {
    ($fmt:expr) => {
        Err(BitTorrentError::DeserdeError(format!($fmt)))
    };
    ($fmt:expr, $($arg:tt)*) => {
        Err(BitTorrentError::DeserdeError(format!($fmt, $($arg)*)))
    };
}

macro_rules! deserialize_int {
    ($($ty:ty)*) => {
        $(
            paste! {
                fn [<deserialize_ $ty>]<V>(self, visitor: V) -> Result<V::Value>
                where
                    V: de::Visitor<'de>,
                {
                    let num_str = self.num_str()?;
                    let num = num_str
                        .parse::<$ty>()
                        .map_err(|e| BitTorrentError::DeserdeError(e.to_string()))?;

                    visitor.[<visit_ $ty>](num)
                }
            }
        )*
    };
}

macro_rules! not_supported {
    ($($ty:ty)*) => {
        $(
            paste! {
                fn [<deserialize_ $ty>]<V>(self, _visitor: V) -> Result<V::Value>
                where
                    V: de::Visitor<'de>,
                {
                    err!("Deserialization of type {} is not supported", stringify!($ty))
                }
            }
        )*
    };
}

#[derive(Debug)]
pub struct Deserializer<R: Read> {
    rdr: BufReader<R>,
}

impl<R: Read> Deserializer<R> {
    pub fn new(rdr: R) -> Self {
        Deserializer {
            rdr: BufReader::new(rdr),
        }
    }

    fn peek(&mut self) -> Result<u8> {
        let buf = self.rdr.fill_buf()?;
        if buf.is_empty() {
            return err!("Unexpected EOF");
        }
        Ok(buf[0])
    }

    fn read_exact(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.rdr.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn read_until(&mut self, byte: u8) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.rdr.read_until(byte, &mut buf)?;
        if buf.is_empty() {
            return err!("Unexpected EOF");
        }
        Ok(buf)
    }

    fn num_str(&mut self) -> Result<String> {
        if self.peek()? != b'i' {
            return err!("Expected integer start 'i'");
        }

        // Consume 'i'
        self.read_exact(1)?;

        let bytes = self.read_until(b'e')?;
        let num_bytes = &bytes[..bytes.len() - 1];

        if num_bytes.is_empty() || is_minus_zero(num_bytes) || has_leading_zeros(num_bytes) {
            return err!("Invalid integer format");
        }

        let num_str = std::str::from_utf8(num_bytes)
            .map_err(deserde_err)?
            .to_string();
        Ok(num_str)
    }

    fn str_or_bytes(&mut self) -> Result<Vec<u8>> {
        let ch = self.peek()?;
        if !ch.is_ascii_digit() {
            return err!("Expected string/bytes length");
        }

        let len_bytes = self.read_until(b':')?;
        let len: usize = std::str::from_utf8(&len_bytes[..len_bytes.len() - 1])
            .map_err(deserde_err)?
            .parse::<usize>()
            .map_err(deserde_err)?;

        self.read_exact(len)
    }
}

impl<'de, R: Read> de::Deserializer<'de> for &mut Deserializer<R> {
    type Error = BitTorrentError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.peek()? {
            b'i' => self.deserialize_i64(visitor),
            b'l' => self.deserialize_seq(visitor),
            b'd' => self.deserialize_map(visitor),
            b'0'..=b'9' => self.deserialize_bytes(visitor),
            _ => err!("Invalid bencode data format"),
        }
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.str_or_bytes()?;
        if bytes.len() != 1 {
            return err!("Expected a single character");
        }
        let ch = bytes[0] as char;
        visitor.visit_char(ch)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.str_or_bytes()?;
        let s = std::str::from_utf8(&bytes).map_err(deserde_err)?;
        visitor.visit_str(s)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.str_or_bytes()?;
        let s = String::from_utf8(bytes).map_err(deserde_err)?;
        visitor.visit_string(s)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.str_or_bytes()?;
        visitor.visit_bytes(&bytes)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.str_or_bytes()?;
        visitor.visit_byte_buf(bytes)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.peek()? {
            b'l' => {
                // Consume 'l'
                self.read_exact(1)?;
                let value = visitor.visit_seq(SeqAccess { de: self })?;
                Ok(value)
            }
            _ => err!("Expected list start 'l' or string/bytes"),
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        if self.peek()? != b'd' {
            return err!("Expected dict start 'd'");
        }

        // Consume 'd'
        self.read_exact(1)?;

        let value = visitor.visit_map(MapAccess { de: self })?;
        Ok(value)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    deserialize_int! { i8 i16 i32 i64 u8 u16 u32 u64 }

    not_supported! { f32 f64 bool unit option }

    forward_to_deserialize_any! {
        unit_struct identifier
        newtype_struct tuple tuple_struct enum ignored_any
    }
}

struct SeqAccess<'a, R: Read> {
    de: &'a mut Deserializer<R>,
}

impl<'de, 'a, R: Read> de::SeqAccess<'de> for SeqAccess<'a, R> {
    type Error = BitTorrentError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        if self.de.peek()? == b'e' {
            // Consume 'e'
            self.de.read_exact(1)?;
            return Ok(None);
        }

        let value = seed.deserialize(&mut *self.de)?;
        Ok(Some(value))
    }
}

struct MapAccess<'a, R: Read> {
    de: &'a mut Deserializer<R>,
}

impl<'de, 'a, R: Read> de::MapAccess<'de> for MapAccess<'a, R> {
    type Error = BitTorrentError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: de::DeserializeSeed<'de>,
    {
        if self.de.peek()? == b'e' {
            // Consume 'e'
            self.de.read_exact(1)?;
            return Ok(None);
        }

        let key = seed.deserialize(&mut *self.de)?;
        Ok(Some(key))
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'de>,
    {
        let value = seed.deserialize(&mut *self.de)?;
        Ok(value)
    }
}

fn is_minus_zero(s: &[u8]) -> bool {
    s == b"-0"
}

fn has_leading_zeros(s: &[u8]) -> bool {
    if s.starts_with(b"-") {
        s.len() > 2 && s[1] == b'0'
    } else {
        s.len() > 1 && s[0] == b'0'
    }
}

fn deserde_err<E: std::error::Error>(e: E) -> BitTorrentError {
    BitTorrentError::DeserdeError(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peek() {
        let data = b"hello";
        let mut deserializer = Deserializer::new(&data[..]);
        let byte = deserializer.peek().unwrap();
        assert_eq!(byte, b'h');

        let byte = deserializer.peek().unwrap();
        assert_eq!(byte, b'h');

        let bytes = deserializer.read_exact(5).unwrap();
        assert_eq!(bytes, b"hello");

        let byte = deserializer.peek();
        assert!(byte.is_err());

        let bytes = deserializer.read_exact(1);
        assert!(bytes.is_err());
    }

    #[test]
    fn test_deserialize_integer() {
        let data = b"i123e";
        let mut deserializer = Deserializer::new(&data[..]);
        let value: i32 = de::Deserialize::deserialize(&mut deserializer).unwrap();
        assert_eq!(value, 123);

        let data = b"i-456e";
        let mut deserializer = Deserializer::new(&data[..]);
        let value: i32 = de::Deserialize::deserialize(&mut deserializer).unwrap();
        assert_eq!(value, -456);

        let data = b"i0e";
        let mut deserializer = Deserializer::new(&data[..]);
        let value: i32 = de::Deserialize::deserialize(&mut deserializer).unwrap();
        assert_eq!(value, 0);

        let data = b"i007e";
        let mut deserializer = Deserializer::new(&data[..]);
        let result: Result<i32> = de::Deserialize::deserialize(&mut deserializer);
        assert!(result.is_err());

        let data = b"i-0e";
        let mut deserializer = Deserializer::new(&data[..]);
        let result: Result<i32> = de::Deserialize::deserialize(&mut deserializer);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_string() {
        let data = b"5:hello";
        let mut deserializer = Deserializer::new(&data[..]);
        let value: String = de::Deserialize::deserialize(&mut deserializer).unwrap();
        assert_eq!(value, "hello");

        let data = b"0:";
        let mut deserializer = Deserializer::new(&data[..]);
        let value: String = de::Deserialize::deserialize(&mut deserializer).unwrap();
        assert_eq!(value, "");
    }

    #[test]
    fn test_deserialize_list() {
        let data = b"l5:helloi123ee";
        let mut deserializer = Deserializer::new(&data[..]);
        let value: Vec<de::IgnoredAny> = de::Deserialize::deserialize(&mut deserializer).unwrap();
        assert_eq!(value.len(), 2);

        let data = b"le";
        let mut deserializer = Deserializer::new(&data[..]);
        let value: Vec<de::IgnoredAny> = de::Deserialize::deserialize(&mut deserializer).unwrap();
        assert_eq!(value.len(), 0);

        let data = b"l5:hello3:byee";
        let mut deserializer = Deserializer::new(&data[..]);
        let value: Vec<String> = de::Deserialize::deserialize(&mut deserializer).unwrap();
        assert_eq!(value, vec!["hello", "bye"]);
    }

    #[test]
    fn test_deserialize_dict() {
        let data = b"d3:foo3:bare";
        let mut deserializer = Deserializer::new(&data[..]);
        let value: std::collections::HashMap<String, String> =
            de::Deserialize::deserialize(&mut deserializer).unwrap();
        assert_eq!(value.get("foo").unwrap(), "bar");

        let data = b"de";
        let mut deserializer = Deserializer::new(&data[..]);
        let value: std::collections::HashMap<String, String> =
            de::Deserialize::deserialize(&mut deserializer).unwrap();
        assert!(value.is_empty());
    }

    #[test]
    fn test_deserialize_struct() {
        use serde::Deserialize;

        #[derive(Deserialize, Debug, PartialEq)]
        struct TestStruct {
            foo: String,
            bar: i32,
        }

        let data = b"d3:foo5:hello3:bari42ee";
        let mut deserializer = Deserializer::new(&data[..]);
        let value: TestStruct = de::Deserialize::deserialize(&mut deserializer).unwrap();
        assert_eq!(
            value,
            TestStruct {
                foo: "hello".to_string(),
                bar: 42
            }
        );
    }
}
