use crate::BitTorrentError;
use serde::de;
use std::marker::PhantomData;

pub(crate) struct ByteSeqVisitor<T>
where
    T: TryFrom<Vec<u8>, Error = BitTorrentError>,
{
    marker: PhantomData<T>,
    unit_len: usize,
}

impl<T> ByteSeqVisitor<T>
where
    T: TryFrom<Vec<u8>, Error = BitTorrentError>,
{
    pub(crate) fn new(unit_len: usize) -> Self {
        ByteSeqVisitor {
            marker: PhantomData,
            unit_len,
        }
    }
}

impl<'de, T> de::Visitor<'de> for ByteSeqVisitor<T>
where
    T: TryFrom<Vec<u8>, Error = BitTorrentError>,
{
    type Value = Vec<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a byte sequence")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if !v.len().is_multiple_of(self.unit_len) {
            return Err(E::custom(
                "byte sequence length is not a multiple of unit length",
            ));
        }

        let mut items = Vec::with_capacity(v.len() / self.unit_len);
        for chunk in v.chunks(self.unit_len) {
            let item = T::try_from(chunk.to_vec()).map_err(E::custom)?;
            items.push(item);
        }
        Ok(items)
    }
}
