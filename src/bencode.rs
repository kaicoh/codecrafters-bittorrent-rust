use regex::Regex;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bencode<'a> {
    Str(&'a str),
    Int(i64),
}

impl<'a> fmt::Display for Bencode<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Str(v) => write!(f, "\"{v}\""),
            Self::Int(v) => write!(f, "{v}"),
        }
    }
}

impl<'a> Bencode<'a> {
    pub fn new(input: &'a str) -> Result<Self, Box<dyn Error>> {
        let re_int = Regex::new(r"^i(?<val>-?\d+)e$").unwrap();
        let re_str = Regex::new(r"^(?<len>\d+):(?<val>.*)").unwrap();

        if let Some(cap) = re_int.captures(input) {
            let val: &str = &cap["val"];
            let re_leading_zeros = Regex::new(r"^-?0+\d+$").unwrap();

            if re_leading_zeros.is_match(val) {
                Err("Bencode::new -- Leading zero is invalid".into())
            } else if val == "-0" {
                Err("Bencode::new -- -0 is invalid".into())
            } else {
                let val = val.parse::<i64>()?;
                Ok(Self::Int(val))
            }
        } else if let Some(cap) = re_str.captures(input) {
            let len = cap["len"].parse::<usize>()?;
            let len_digits = len / 10 + 1;
            let str_start_at = len_digits + 1;
            let str_end_at = str_start_at + len;
            Ok(Self::Str(&input[str_start_at..str_end_at]))
        } else {
            Err("unknown input".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_encodes_positive_int() {
        let input = "i3e";
        let val = Bencode::new(input).unwrap();
        assert_eq!(val, Bencode::Int(3));

        let input = "i12e";
        let val = Bencode::new(input).unwrap();
        assert_eq!(val, Bencode::Int(12));

        let input = "i0e";
        let val = Bencode::new(input).unwrap();
        assert_eq!(val, Bencode::Int(0));

        let input = "i00e";
        let err = Bencode::new(input);
        assert!(err.is_err());
    }

    #[test]
    fn it_encodes_negative_int() {
        let input = "i-3e";
        let val = Bencode::new(input).unwrap();
        assert_eq!(val, Bencode::Int(-3));

        let input = "i-12e";
        let val = Bencode::new(input).unwrap();
        assert_eq!(val, Bencode::Int(-12));

        let input = "i-0e";
        let err = Bencode::new(input);
        assert!(err.is_err());
    }

    #[test]
    fn it_encodes_string() {
        let input = "5:hello";
        let val = Bencode::new(input).unwrap();
        assert_eq!(val, Bencode::Str("hello"));

        let input = "0:";
        let val = Bencode::new(input).unwrap();
        assert_eq!(val, Bencode::Str(""));
    }
}
