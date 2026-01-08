use super::{Bencode, BitTorrentError};

use serde::ser::{self, Error as SerdeError, SerializeMap as SerdeMap, SerializeSeq as SerdeSeq};
use std::collections::HashMap;
use std::io;

impl SerdeError for BitTorrentError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        BitTorrentError::SerdeError(format!("{msg}"))
    }
}

impl ser::Serialize for Bencode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match self {
            Bencode::Str(v) => serializer.serialize_bytes(v),
            Bencode::Int(v) => serializer.serialize_i64(*v),
            Bencode::List(vals) => {
                let mut seq = serializer.serialize_seq(Some(vals.len()))?;
                for val in vals {
                    seq.serialize_element(val)?;
                }
                seq.end()
            }
            Bencode::Dict(map) => {
                let mut ser_map = serializer.serialize_map(Some(map.len()))?;
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                for key in keys {
                    ser_map.serialize_entry(key, &map[key])?;
                }
                ser_map.end()
            }
        }
    }
}

pub struct Serializer<W> {
    writer: W,
}

pub struct SerializeSeq<'a, W: io::Write> {
    serializer: &'a mut Serializer<W>,
    first: bool,
}

pub struct SerializeMap<'a, W>
where
    W: io::Write,
{
    serializer: &'a mut Serializer<W>,
    inner: HashMap<Vec<u8>, Vec<u8>>,
}

impl<W: io::Write> Serializer<W> {
    pub fn new(writer: W) -> Self {
        Serializer { writer }
    }
}

impl<'a, W: io::Write> ser::Serializer for &'a mut Serializer<W> {
    type Ok = ();
    type Error = BitTorrentError;

    type SerializeSeq = SerializeSeq<'a, W>;
    type SerializeTuple = SerializeSeq<'a, W>;
    type SerializeTupleStruct = SerializeSeq<'a, W>;
    type SerializeTupleVariant = SerializeSeq<'a, W>;
    type SerializeMap = SerializeMap<'a, W>;
    type SerializeStruct = SerializeMap<'a, W>;
    type SerializeStructVariant = SerializeMap<'a, W>;

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(BitTorrentError::SerdeError(
            "Bencode does not support boolean type".into(),
        ))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        write!(self.writer, "i{}e", v)?;
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        write!(self.writer, "i{}e", v)?;
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(v as f64)
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(BitTorrentError::SerdeError(
            "Bencode does not support float type".into(),
        ))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        write!(self.writer, "{}:{}", v.len_utf8(), v)?;
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        write!(self.writer, "{}:{}", v.len(), v)?;
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        write!(self.writer, "{}:", v.len())?;
        self.writer.write_all(v)?;
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(BitTorrentError::SerdeError(
            "Bencode does not support None type".into(),
        ))
    }

    fn serialize_some<T: ?Sized + ser::Serialize>(
        self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(BitTorrentError::SerdeError(
            "Bencode does not support unit type".into(),
        ))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(BitTorrentError::SerdeError(
            "Bencode does not support unit struct type".into(),
        ))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(BitTorrentError::SerdeError(
            "Bencode does not support unit variant type".into(),
        ))
    }

    fn serialize_newtype_struct<T: ?Sized + ser::Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + ser::Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SerializeSeq {
            serializer: self,
            first: true,
        })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(SerializeSeq {
            serializer: self,
            first: true,
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(SerializeSeq {
            serializer: self,
            first: true,
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(SerializeSeq {
            serializer: self,
            first: true,
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(SerializeMap {
            serializer: self,
            inner: HashMap::new(),
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(SerializeMap {
            serializer: self,
            inner: HashMap::new(),
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(SerializeMap {
            serializer: self,
            inner: HashMap::new(),
        })
    }
}

impl<'a, W: io::Write> SerializeSeq<'a, W> {
    fn write_beginning(&mut self) -> Result<(), BitTorrentError> {
        if self.first {
            self.serializer.writer.write_all(b"l")?;
            self.first = false;
        }
        Ok(())
    }
}

impl<'a, W: io::Write> ser::SerializeSeq for SerializeSeq<'a, W> {
    type Ok = ();
    type Error = BitTorrentError;

    fn serialize_element<T: ?Sized + ser::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.write_beginning()?;
        value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut val = self;
        val.write_beginning()?;
        val.serializer.writer.write_all(b"e")?;
        Ok(())
    }
}

impl<'a, W: io::Write> ser::SerializeTuple for SerializeSeq<'a, W> {
    type Ok = ();
    type Error = BitTorrentError;

    fn serialize_element<T: ?Sized + ser::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W: io::Write> ser::SerializeTupleStruct for SerializeSeq<'a, W> {
    type Ok = ();
    type Error = BitTorrentError;

    fn serialize_field<T: ?Sized + ser::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W: io::Write> ser::SerializeTupleVariant for SerializeSeq<'a, W> {
    type Ok = ();
    type Error = BitTorrentError;

    fn serialize_field<T: ?Sized + ser::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W: io::Write> ser::SerializeMap for SerializeMap<'a, W> {
    type Ok = ();
    type Error = BitTorrentError;

    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<(), Self::Error>
    where
        K: ?Sized + ser::Serialize,
        V: ?Sized + ser::Serialize,
    {
        let mut key_bytes: Vec<u8> = Vec::new();
        key.serialize(&mut Serializer::new(&mut key_bytes))?;

        let mut value_bytes: Vec<u8> = Vec::new();
        value.serialize(&mut Serializer::new(&mut value_bytes))?;

        self.inner.insert(key_bytes, value_bytes);
        Ok(())
    }

    fn serialize_key<T: ?Sized + ser::Serialize>(&mut self, _: &T) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_value<T: ?Sized + ser::Serialize>(&mut self, _: &T) -> Result<(), Self::Error> {
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.serializer.writer.write_all(b"d")?;

        let mut keys: Vec<&Vec<u8>> = self.inner.keys().collect();
        keys.sort_by_key(|k| str_part(k));

        for key in keys {
            self.serializer.writer.write_all(key)?;
            self.serializer.writer.write_all(&self.inner[key])?;
        }

        self.serializer.writer.write_all(b"e")?;
        Ok(())
    }
}

impl<'a, W: io::Write> ser::SerializeStruct for SerializeMap<'a, W> {
    type Ok = ();
    type Error = BitTorrentError;

    fn serialize_field<T: ?Sized + ser::Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        ser::SerializeMap::serialize_entry(self, &key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeMap::end(self)
    }
}

impl<'a, W: io::Write> ser::SerializeStructVariant for SerializeMap<'a, W> {
    type Ok = ();
    type Error = BitTorrentError;

    fn serialize_field<T: ?Sized + ser::Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        ser::SerializeMap::serialize_entry(self, &key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeMap::end(self)
    }
}

fn str_part(s: &[u8]) -> &[u8] {
    let mut iter = s.splitn(2, |&b| b == b':');
    iter.next();
    iter.next().unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bencode::Bencode;
    use serde::Serialize;

    #[test]
    fn test_str_part() {
        let s = b"4:spam";
        assert_eq!(str_part(s), b"spam");
    }

    #[test]
    fn test_serialize_str() {
        let mut buf = Vec::new();
        let mut serializer = Serializer::new(&mut buf);
        let val = Bencode::Str(b"spam".to_vec());
        val.serialize(&mut serializer).unwrap();
        assert_eq!(buf, b"4:spam");

        let mut buf = Vec::new();
        let mut serializer = Serializer::new(&mut buf);
        let val = "spam";
        val.serialize(&mut serializer).unwrap();
        assert_eq!(buf, b"4:spam");
    }

    #[test]
    fn test_serialize_int() {
        let mut buf = Vec::new();
        let mut serializer = Serializer::new(&mut buf);
        let val = Bencode::Int(-42);
        val.serialize(&mut serializer).unwrap();
        assert_eq!(buf, b"i-42e");

        let mut buf = Vec::new();
        let mut serializer = Serializer::new(&mut buf);
        let val = -42;
        val.serialize(&mut serializer).unwrap();
        assert_eq!(buf, b"i-42e");
    }

    #[test]
    fn test_serialize_list() {
        let mut buf = Vec::new();
        let mut serializer = Serializer::new(&mut buf);
        let val = Bencode::List(vec![Bencode::Int(1), Bencode::Str(b"spam".to_vec())]);
        val.serialize(&mut serializer).unwrap();
        assert_eq!(buf, b"li1e4:spame");

        let mut buf = Vec::new();
        let mut serializer = Serializer::new(&mut buf);
        let val = vec![1, 2, 3];
        val.serialize(&mut serializer).unwrap();
        assert_eq!(buf, b"li1ei2ei3ee");
    }

    #[test]
    fn test_serialize_dict() {
        let mut buf = Vec::new();
        let mut serializer = Serializer::new(&mut buf);

        let mut dict = HashMap::new();
        dict.insert("name".into(), Bencode::Str(b"Alice".to_vec()));
        dict.insert("age".into(), Bencode::Int(30));

        let val = Bencode::Dict(dict);
        val.serialize(&mut serializer).unwrap();
        assert_eq!(buf, b"d3:agei30e4:name5:Alicee");

        #[derive(Serialize)]
        struct Test {
            name: String,
            age: u8,
        }
        let mut buf = Vec::new();
        let mut serializer = Serializer::new(&mut buf);
        let val = Test {
            name: "Alice".into(),
            age: 30,
        };
        val.serialize(&mut serializer).unwrap();
        assert_eq!(buf, b"d3:agei30e4:name5:Alicee");

        #[derive(Serialize)]
        struct Nested {
            link: String,
            info: Test,
        }
        let mut buf = Vec::new();
        let mut serializer = Serializer::new(&mut buf);
        let val = Nested {
            link: "http://example.com/announce".into(),
            info: Test {
                name: "Alice".into(),
                age: 30,
            },
        };
        val.serialize(&mut serializer).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "d4:infod3:agei30e4:name5:Alicee4:link27:http://example.com/announcee"
        );
    }
}
