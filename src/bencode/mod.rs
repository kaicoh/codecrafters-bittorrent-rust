mod ser;
pub use ser::Serializer;

use crate::{BitTorrentError, Result};

use serde::ser::{Serialize, SerializeMap, SerializeSeq};
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read};

macro_rules! bail_if {
    ($cond:expr, $err:expr) => {
        if $cond {
            return Err(BitTorrentError::BencodeError($err));
        }
    };
}

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

impl<'a> From<&'a str> for Bencode {
    fn from(value: &'a str) -> Self {
        Bencode::Str(value.as_bytes().to_vec())
    }
}

impl From<i64> for Bencode {
    fn from(value: i64) -> Self {
        Bencode::Int(value)
    }
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

impl TryFrom<String> for Bencode {
    type Error = BitTorrentError;

    fn try_from(value: String) -> Result<Self> {
        Self::parse(value.as_bytes())
    }
}

impl Bencode {
    pub fn parse(input: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(input);
        let c = match cursor.next_char() {
            Some(ch) => ch,
            None => bail!("Empty input"),
        };
        Self::get_from_cursor(&mut cursor, c, "Invalid bencode format")
    }

    pub fn as_str(&self) -> Result<&[u8]> {
        match self {
            Bencode::Str(v) => Ok(v),
            _ => bail!("Bencode value is not a string"),
        }
    }

    pub fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        Self::parse(&buffer)
    }

    pub fn as_dict(&self) -> Result<BencodeDict> {
        match self {
            Bencode::Dict(items) => Ok(BencodeDict {
                inner: items.clone(),
            }),
            _ => bail!("Bencode value is not a dictionary"),
        }
    }

    fn new_int(cursor: &mut Cursor<'_>) -> Result<Self> {
        let bytes = cursor.read_until('e')?;
        bail_if!(
            bytes.is_empty() || minus_zero(bytes) || leading_zeros(bytes),
            "Invalid integer encoding"
        );
        let int_val: i64 = std::str::from_utf8(bytes)?.parse()?;
        Ok(Bencode::Int(int_val))
    }

    fn new_str(cursor: &mut Cursor<'_>, first_char: char) -> Result<Self> {
        let mut len_str = String::new();
        len_str.push(first_char);

        loop {
            match cursor.next_char() {
                Some(ch) if ch.is_ascii_digit() => len_str.push(ch),
                Some(':') => break,
                _ => bail!("Invalid string length encoding"),
            }
        }

        let str_len: usize = len_str.parse()?;
        let str_bytes = cursor.read_exact(str_len)?;

        Ok(Bencode::Str(str_bytes.to_vec()))
    }

    fn new_list(cursor: &mut Cursor<'_>) -> Result<Self> {
        let mut items: Vec<Self> = Vec::new();

        match cursor.next_char() {
            Some('e') => Ok(Bencode::List(items)),
            _ => {
                cursor.step_back();

                loop {
                    match cursor.next_char() {
                        Some('e') => break,
                        Some(c) => {
                            let item = Self::get_from_cursor(
                                cursor,
                                c,
                                "Invalid bencode format in list item",
                            )?;

                            items.push(item);
                        }
                        None => bail!("Unexpected end of input in list"),
                    }
                }
                Ok(Bencode::List(items))
            }
        }
    }

    fn new_dict(cursor: &mut Cursor<'_>) -> Result<Self> {
        let mut items: HashMap<String, Self> = HashMap::new();

        match cursor.next_char() {
            Some('e') => Ok(Bencode::Dict(items)),
            _ => {
                cursor.step_back();

                loop {
                    match cursor.next_char() {
                        Some('e') => break,
                        Some(c) => {
                            let key_bencode = Self::get_from_cursor(
                                cursor,
                                c,
                                "Invalid bencode format in dict key",
                            )?;

                            let key = match key_bencode {
                                Bencode::Str(s) => String::from_utf8(s)?,
                                _ => bail!("Dictionary keys must be strings"),
                            };

                            let value_first_char = match cursor.next_char() {
                                Some(ch) => ch,
                                None => bail!("Unexpected end of input in dict value"),
                            };

                            let value = Self::get_from_cursor(
                                cursor,
                                value_first_char,
                                "Invalid bencode format in dict value",
                            )?;

                            items.insert(key, value);
                        }
                        None => bail!("Unexpected end of input in dict"),
                    }
                }
                Ok(Bencode::Dict(items))
            }
        }
    }

    fn get_from_cursor(
        cursor: &mut Cursor<'_>,
        first_char: char,
        msg: &'static str,
    ) -> Result<Self> {
        match first_char {
            'i' => Self::new_int(cursor),
            'l' => Self::new_list(cursor),
            'd' => Self::new_dict(cursor),
            c if c.is_ascii_digit() => Self::new_str(cursor, c),
            _ => bail!(msg),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BencodeDict {
    inner: HashMap<String, Bencode>,
}

impl BencodeDict {
    pub fn get(&self, key: &str) -> Result<&Bencode> {
        match self.inner.get(key) {
            Some(v) => Ok(v),
            None => bail!("Key not found"),
        }
    }

    pub fn get_str(&self, key: &str) -> Result<&str> {
        match self.inner.get(key) {
            Some(Bencode::Str(v)) => Ok(std::str::from_utf8(v)?),
            _ => bail!("Key not found or not a string"),
        }
    }

    pub fn get_bytes(&self, key: &str) -> Result<&[u8]> {
        match self.inner.get(key) {
            Some(Bencode::Str(v)) => Ok(v),
            _ => bail!("Key not found or not a string"),
        }
    }

    pub fn get_int(&self, key: &str) -> Result<i64> {
        match self.inner.get(key) {
            Some(Bencode::Int(v)) => Ok(*v),
            _ => bail!("Key not found or not an integer"),
        }
    }
}

#[derive(Debug)]
struct Cursor<'a> {
    inner: io::Cursor<&'a [u8]>,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            inner: io::Cursor::new(input),
        }
    }

    fn step_back(&mut self) {
        let pos = self.inner.position();
        if pos > 0 {
            self.inner.set_position(pos - 1);
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let mut buf = [0; 1];
        match self.inner.read_exact(&mut buf) {
            Ok(_) => Some(buf[0] as char),
            Err(_) => None,
        }
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8]> {
        let start_pos = self.inner.position() as usize;
        let end_pos = start_pos + len;

        let mut buf = vec![0; len];
        self.inner.read_exact(&mut buf)?;

        Ok(&self.inner.get_ref()[start_pos..end_pos])
    }

    fn read_until(&mut self, del: char) -> Result<&'a [u8]> {
        let start_pos = self.inner.position() as usize;
        let mut end_pos = start_pos;

        loop {
            match self.next_char() {
                Some(c) if c == del => break,
                Some(_) => end_pos += 1,
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Delimiter not found",
                    )
                    .into());
                }
            }
        }

        Ok(&self.inner.get_ref()[start_pos..end_pos])
    }
}

impl Serialize for Bencode {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        match self {
            Self::Int(i) => serializer.serialize_i64(*i),
            Self::Str(bytes) => serializer.serialize_bytes(bytes),
            Self::List(list) => {
                let mut seq = serializer.serialize_seq(Some(list.len()))?;
                for item in list {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Self::Dict(map) => {
                let mut ser_map = serializer.serialize_map(Some(map.len()))?;

                let mut sorted_keys: Vec<&String> = map.keys().collect();
                sorted_keys.sort();

                for key in sorted_keys {
                    ser_map.serialize_entry(key, &map[key])?;
                }

                ser_map.end()
            }
        }
    }
}

fn minus_zero(s: &[u8]) -> bool {
    s == b"-0"
}

fn leading_zeros(s: &[u8]) -> bool {
    if s.starts_with(b"-") {
        s.len() > 2 && s[1] == b'0'
    } else {
        s.len() > 1 && s[0] == b'0'
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_encodes_positive_int() {
        let input = b"i3e";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Int(3));

        let input = b"i12e";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Int(12));

        let input = b"i0e";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Int(0));

        let input = b"i00e";
        let err = Bencode::parse(input);
        assert!(err.is_err());
    }

    #[test]
    fn it_encodes_negative_int() {
        let input = b"i-3e";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Int(-3));

        let input = b"i-12e";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Int(-12));

        let input = b"i-0e";
        let err = Bencode::parse(input);
        assert!(err.is_err());
    }

    #[test]
    fn it_encodes_string() {
        let input = b"5:hello";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Str("hello".into()));

        let input = b"0:";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Str("".into()));
    }

    #[test]
    fn it_encodes_list() {
        let input = b"l4:spam4:eggse";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(
            val,
            Bencode::List(vec![
                Bencode::Str("spam".into()),
                Bencode::Str("eggs".into()),
            ])
        );

        let input = b"l5:helloi52ee";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(
            val,
            Bencode::List(vec![Bencode::Str("hello".into()), Bencode::Int(52),])
        );

        let input = b"le";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::List(vec![]));
    }

    #[test]
    fn it_encodes_dict() {
        let input = b"d3:foo3:bar3:bazi42ee";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(
            val,
            Bencode::Dict(
                [
                    ("foo".into(), Bencode::Str("bar".into())),
                    ("baz".into(), Bencode::Int(42)),
                ]
                .into()
            )
        );

        let input = b"de";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Dict([].into()));
    }

    #[test]
    fn it_displays_bencode() {
        let bencode_int = Bencode::Int(42);
        assert_eq!(bencode_int.to_string(), "42");

        let bencode_str = Bencode::Str("hello".into());
        assert_eq!(bencode_str.to_string(), "\"hello\"");

        let bencode_list = Bencode::List(vec![Bencode::Int(1), Bencode::Str("two".into())]);
        assert_eq!(bencode_list.to_string(), "[1,\"two\"]");

        let bencode_dict = Bencode::Dict(
            [
                ("foo".into(), Bencode::Str("bar".into())),
                ("baz".into(), Bencode::Int(123)),
            ]
            .into(),
        );
        assert_eq!(bencode_dict.to_string(), "{\"baz\":123,\"foo\":\"bar\"}");
    }
}
