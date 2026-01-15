use crate::{Result, bencode::Bencode, meta::MagnetLink, net::Extension, util::Bytes20};

use super::utils;
use std::collections::HashMap;
use std::str::FromStr;

pub(crate) async fn run(url: String) -> Result<()> {
    let magnet_link = MagnetLink::from_str(&url)?;
    let resp = utils::get_response(&magnet_link).await?;

    for peer in resp.peers {
        let info_hash = magnet_link.info_hash();
        let peer_id = Bytes20::new(*b"-CT0001-012345678901");

        let mut stream = peer.connect(info_hash, peer_id).await?;
        stream.wait_bitfield().await?;
        stream.send_message(msg_payload()).await?;

        println!("Peer ID: {}", stream.peer_id().hex_encoded());
    }

    Ok(())
}

fn msg_payload() -> Extension {
    let mut dict = HashMap::new();
    dict.insert(
        "m".to_string(),
        Bencode::Dict({
            let mut ext_map = HashMap::new();
            ext_map.insert("ut_metadata".to_string(), Bencode::Int(1));
            ext_map
        }),
    );
    Extension::Handshake(dict)
}
