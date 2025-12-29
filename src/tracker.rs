use crate::{BitTorrentError, Result, bencode::Bencode, peer::Peer, util::Bytes20};

use std::borrow::Cow;
use url::EncodingOverride;

macro_rules! err {
    ($msg:expr) => {
        BitTorrentError::TrackerError($msg)
    };
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
        let bencode = Bencode::parse(&resp)?;
        let response = TrackerResponse::try_from(&bencode)?;
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

#[derive(Debug, Clone)]
pub struct TrackerResponse {
    pub interval: u64,
    pub peers: Vec<Peer>,
}

impl TryFrom<&Bencode> for TrackerResponse {
    type Error = BitTorrentError;

    fn try_from(value: &Bencode) -> Result<Self> {
        let dict = value.as_dict()?;
        let interval = dict.get_int("interval")? as u64;
        let peers = dict.get("peers")?.try_into()?;
        Ok(Self { interval, peers })
    }
}
