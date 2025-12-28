use crate::{BitTorrentError, Result};

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
pub enum Bencode<'a> {
    Str(&'a str),
    Int(i64),
    List(Vec<Bencode<'a>>),
}

impl<'a> fmt::Display for Bencode<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Str(v) => write!(f, "\"{v}\""),
            Self::Int(v) => write!(f, "{v}"),
            Self::List(vals) => write!(
                f,
                "[{}]",
                vals.iter()
                    .map(|v| format!("{v}"))
                    .collect::<Vec<String>>()
                    .join(",")
            ),
        }
    }
}

impl<'a> TryFrom<&'a str> for Bencode<'a> {
    type Error = BitTorrentError;

    fn try_from(value: &'a str) -> Result<Self> {
        let mut cursor = Cursor::new(value);
        match cursor.next_char() {
            Some('i') => Self::new_int(&mut cursor),
            Some('l') => Self::new_list(&mut cursor),
            Some(c) if c.is_ascii_digit() => Self::new_str(&mut cursor, c),
            _ => bail!("Invalid bencode format"),
        }
    }
}

impl<'a> Bencode<'a> {
    pub fn parse(input: &'a str) -> Result<Self> {
        Self::try_from(input)
    }

    fn new_int(cursor: &mut Cursor<'a>) -> Result<Self> {
        let int_str = cursor.read_until('e')?;
        bail_if!(
            int_str.is_empty() || minus_zero(int_str) || leading_zeros(int_str),
            "Invalid integer encoding"
        );
        let int_val: i64 = int_str.parse()?;
        Ok(Bencode::Int(int_val))
    }

    fn new_str(cursor: &mut Cursor<'a>, first_char: char) -> Result<Self> {
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
        let str_val = cursor.read_exact(str_len)?;

        Ok(Bencode::Str(str_val))
    }

    fn new_list(cursor: &mut Cursor<'a>) -> Result<Self> {
        let mut items: Vec<Self> = Vec::new();

        match cursor.next_char() {
            Some('e') => Ok(Bencode::List(items)),
            _ => {
                cursor.step_back();

                loop {
                    match cursor.next_char() {
                        Some('e') => break,
                        Some(c) => {
                            let item = match c {
                                'i' => Self::new_int(cursor)?,
                                'l' => Self::new_list(cursor)?,
                                c if c.is_ascii_digit() => Self::new_str(cursor, c)?,
                                _ => bail!("Invalid bencode format in list"),
                            };

                            items.push(item);
                        }
                        None => bail!("Unexpected end of input in list"),
                    }
                }
                Ok(Bencode::List(items))
            }
        }
    }
}

#[derive(Debug)]
struct Cursor<'a> {
    inner: std::io::Cursor<&'a str>,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            inner: std::io::Cursor::new(input),
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

    fn read_exact(&mut self, len: usize) -> Result<&'a str> {
        let start_pos = self.inner.position() as usize;
        let end_pos = start_pos + len;

        let mut buf = vec![0; len];
        self.inner.read_exact(&mut buf)?;

        Ok(&self.inner.get_ref()[start_pos..end_pos])
    }

    fn read_until(&mut self, del: char) -> Result<&'a str> {
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

fn minus_zero(s: &str) -> bool {
    s == "-0"
}

fn leading_zeros(s: &str) -> bool {
    let trimmed = s.trim_start_matches('-');
    trimmed.len() > 1 && trimmed.starts_with('0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_encodes_positive_int() {
        let input = "i3e";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Int(3));

        let input = "i12e";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Int(12));

        let input = "i0e";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Int(0));

        let input = "i00e";
        let err = Bencode::parse(input);
        assert!(err.is_err());
    }

    #[test]
    fn it_encodes_negative_int() {
        let input = "i-3e";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Int(-3));

        let input = "i-12e";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Int(-12));

        let input = "i-0e";
        let err = Bencode::parse(input);
        assert!(err.is_err());
    }

    #[test]
    fn it_encodes_string() {
        let input = "5:hello";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Str("hello"));

        let input = "0:";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::Str(""));
    }

    #[test]
    fn it_encodes_list() {
        let input = "l4:spam4:eggse";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(
            val,
            Bencode::List(vec![Bencode::Str("spam"), Bencode::Str("eggs"),])
        );

        let input = "l5:helloi52ee";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(
            val,
            Bencode::List(vec![Bencode::Str("hello"), Bencode::Int(52),])
        );

        let input = "le";
        let val = Bencode::parse(input).unwrap();
        assert_eq!(val, Bencode::List(vec![]));
    }

    #[test]
    fn it_displays_bencode() {
        let bencode_int = Bencode::Int(42);
        assert_eq!(bencode_int.to_string(), "42");

        let bencode_str = Bencode::Str("hello");
        assert_eq!(bencode_str.to_string(), "\"hello\"");

        let bencode_list = Bencode::List(vec![Bencode::Int(1), Bencode::Str("two")]);
        assert_eq!(bencode_list.to_string(), "[1,\"two\"]");
    }
}
