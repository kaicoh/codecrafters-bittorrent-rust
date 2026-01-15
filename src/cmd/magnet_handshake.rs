use crate::{Result, meta::MagnetLink, util::Bytes20};

use super::utils;
use std::str::FromStr;

pub(crate) async fn run(url: String) -> Result<()> {
    let magnet_link = MagnetLink::from_str(&url)?;
    let resp = utils::get_response(&magnet_link).await?;

    for peer in resp.peers {
        let info_hash = magnet_link.info_hash();
        let peer_id = Bytes20::new(*b"-CT0001-012345678901");

        let stream = peer.connect(info_hash, peer_id).await?;

        println!("Peer ID: {}", stream.peer_id().hex_encoded());
    }

    Ok(())
}
