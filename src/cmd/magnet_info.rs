use crate::{meta::MagnetLink, net::Extension, util::Bytes20};

use super::utils;
use std::error::Error;
use std::str::FromStr;

pub(crate) async fn run(url: String) -> Result<(), Box<dyn Error>> {
    let magnet_link = MagnetLink::from_str(&url)?;
    let resp = utils::get_response(&magnet_link).await?;

    for peer in resp.peers {
        let info_hash = magnet_link.info_hash();
        let peer_id = Bytes20::new(*b"-CT0001-012345678901");

        let mut stream = peer.connect(info_hash, peer_id).await?;
        println!("Peer ID: {}", stream.peer_id().hex_encoded());

        let ext_id = stream
            .extension_handshake()
            .await?
            .metadata_ext_id()
            .ok_or("Peer did not advertise ut_metadata extension")?;

        println!("Peer Metadata Extension ID: {ext_id}");

        stream
            .send_message(Extension::RequestMetadata { ext_id, piece: 0 })
            .await?;
    }

    Ok(())
}
