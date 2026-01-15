use crate::{BitTorrentError, util::Bytes20};

use super::{AsTrackerRequest, TrackerRequest};

use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct MagnetLink {
    info_hash: Vec<u8>,
    name: Option<String>,
    tracker: Option<String>,
}

impl MagnetLink {
    pub fn info_hash(&self) -> Bytes20 {
        Bytes20::from(self.info_hash.as_ref())
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn tracker(&self) -> Option<&str> {
        self.tracker.as_deref()
    }
}

impl FromStr for MagnetLink {
    type Err = BitTorrentError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with("magnet:?") {
            return Err(BitTorrentError::InvalidMagnetLink);
        }

        let query = &s[8..];

        let params = serde_urlencoded::from_str::<HashMap<String, String>>(query)?;

        let info_hash = params
            .get("xt")
            .and_then(|xt| xt.strip_prefix("urn:btih:"))
            .map(hex::decode)
            .transpose()?
            .ok_or_else(|| BitTorrentError::InvalidMagnetLink)?;

        let name = params.get("dn").cloned();
        let tracker = params.get("tr").cloned();

        Ok(MagnetLink {
            info_hash,
            name,
            tracker,
        })
    }
}

impl AsTrackerRequest for MagnetLink {
    fn as_tracker_request(&self) -> crate::Result<TrackerRequest> {
        TrackerRequest::builder()
            .url(
                self.tracker
                    .as_deref()
                    .ok_or(BitTorrentError::InvalidMagnetLink)?,
            )
            .info_hash(self.info_hash())
            .left(999)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magnet_link_parsing() {
        let magnet_str = "magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&dn=magnet1.gif&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce";
        let magnet_link = MagnetLink::from_str(magnet_str).unwrap();
        assert_eq!(
            magnet_link,
            MagnetLink {
                info_hash: hex::decode("ad42ce8109f54c99613ce38f9b4d87e70f24a165").unwrap(),
                name: Some("magnet1.gif".to_string()),
                tracker: Some(
                    "http://bittorrent-test-tracker.codecrafters.io/announce".to_string()
                ),
            }
        );
    }
}
