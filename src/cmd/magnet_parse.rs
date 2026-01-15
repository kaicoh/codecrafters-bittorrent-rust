use crate::{BitTorrentError, Result};

use std::collections::HashMap;
use std::str::FromStr;

pub(crate) async fn run(url: String) -> Result<()> {
    let magnet_link = MagnetLink::from_str(&url)?;
    println!(
        "Tracker URL: {}",
        magnet_link.tracker.unwrap_or("N/A".to_string())
    );
    println!("Info Hash: {}", magnet_link.info_hash);
    Ok(())
}

#[derive(Debug, PartialEq)]
struct MagnetLink {
    info_hash: String,
    name: Option<String>,
    tracker: Option<String>,
}

impl FromStr for MagnetLink {
    type Err = BitTorrentError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if !s.starts_with("magnet:?") {
            return Err(BitTorrentError::InvalidMagnetLink);
        }

        let query = &s[8..];

        let params = serde_urlencoded::from_str::<HashMap<String, String>>(query)?;

        let info_hash = params
            .get("xt")
            .and_then(|xt| xt.strip_prefix("urn:btih:").map(|s| s.to_string()))
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
                info_hash: "ad42ce8109f54c99613ce38f9b4d87e70f24a165".to_string(),
                name: Some("magnet1.gif".to_string()),
                tracker: Some(
                    "http://bittorrent-test-tracker.codecrafters.io/announce".to_string()
                ),
            }
        );
    }
}
