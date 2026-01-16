use crate::{
    BitTorrentError, Result,
    bencode::{ByteSeqVisitor, Deserializer},
    net::{PEER_BYTE_SIZE, Peer},
    util::Bytes20,
};

use serde::{Deserialize, de};
use std::borrow::Cow;
use std::ops::Deref;
use url::EncodingOverride;

macro_rules! err {
    ($msg:expr) => {
        BitTorrentError::TrackerError($msg)
    };
}

pub trait AsTrackerRequest {
    fn as_tracker_request(&self) -> Result<TrackerRequest>;
}

#[derive(Debug)]
pub struct TrackerRequest {
    inner: reqwest::RequestBuilder,
}

impl TrackerRequest {
    pub fn builder() -> TrackerRequestBuilder {
        TrackerRequestBuilder::default()
    }

    pub async fn send(self) -> Result<TrackerResponse> {
        let resp = self.inner.send().await?.bytes().await?;
        let mut de = Deserializer::new(resp.deref());
        let response = Deserialize::deserialize(&mut de)?;
        Ok(response)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackerRequestBuilder {
    url: Option<String>,
    info_hash: Option<Bytes20>,
    peer_id: Option<String>,
    port: Option<u16>,
    uploaded: Option<u64>,
    downloaded: Option<u64>,
    left: Option<u64>,
    compact: Option<u8>,
}

impl TrackerRequestBuilder {
    pub fn build(self) -> Result<TrackerRequest> {
        let mut url = self
            .url
            .as_deref()
            .map(reqwest::Url::parse)
            .transpose()?
            .ok_or(err!("url is required by RequestBuilder"))?;

        let info_hash = self
            .info_hash
            .ok_or(err!("info_hash is required by RequestBuilder"))?;

        let unsafe_hash_str = unsafe { std::str::from_utf8_unchecked(info_hash.as_ref()) };

        let peer_id = self.peer_id.as_deref().unwrap_or("01234567890123456789");

        let port = self.port.unwrap_or(6881).to_string();
        let uploaded = self.uploaded.unwrap_or(0).to_string();
        let downloaded = self.downloaded.unwrap_or(0).to_string();

        let left = self
            .left
            .ok_or(err!("left is required by RequestBuilder"))?
            .to_string();

        let compact = self.compact.unwrap_or(1).to_string();

        let encoding: EncodingOverride<'_> = Some(&|v| {
            if v == unsafe_hash_str {
                Cow::Owned(v.as_bytes().to_vec())
            } else {
                Cow::Borrowed(v.as_bytes())
            }
        });

        let url = url
            .query_pairs_mut()
            .encoding_override(encoding)
            .append_pair("info_hash", unsafe_hash_str)
            .append_pair("peer_id", peer_id)
            .append_pair("port", &port)
            .append_pair("uploaded", &uploaded)
            .append_pair("downloaded", &downloaded)
            .append_pair("left", &left)
            .append_pair("compact", &compact)
            .finish();

        let req = reqwest::Client::new().get(url.as_str());
        Ok(TrackerRequest { inner: req })
    }

    pub fn url(self, url: impl Into<String>) -> Self {
        Self {
            url: Some(url.into()),
            ..self
        }
    }

    pub fn info_hash<T: AsRef<[u8]>>(self, info_hash: T) -> Self {
        Self {
            info_hash: Some(Bytes20::from(info_hash.as_ref())),
            ..self
        }
    }

    pub fn left(self, left: u64) -> Self {
        Self {
            left: Some(left),
            ..self
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrackerResponse {
    pub interval: u64,
    pub peers: Peers,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Peers(Vec<Peer>);

impl Peers {
    pub fn iter(&self) -> std::slice::Iter<'_, Peer> {
        self.0.iter()
    }
}

impl AsRef<[Peer]> for Peers {
    fn as_ref(&self) -> &[Peer] {
        &self.0
    }
}

impl<'de> de::Deserialize<'de> for Peers {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let visitor = ByteSeqVisitor::new(PEER_BYTE_SIZE);
        let vec = deserializer.deserialize_bytes(visitor)?;
        Ok(Self(vec))
    }
}

impl IntoIterator for Peers {
    type Item = Peer;
    type IntoIter = std::vec::IntoIter<Peer>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
