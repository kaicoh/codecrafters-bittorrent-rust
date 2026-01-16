use crate::{
    bencode::Deserializer,
    meta::{Info, MagnetLink},
    net::Extension,
    util::Bytes20,
};

use super::utils;
use serde::Deserialize;
use std::error::Error;
use std::str::FromStr;

pub(crate) async fn run(url: String) -> Result<(), Box<dyn Error>> {
    let magnet_link = MagnetLink::from_str(&url)?;
    println!("Tracker URL: {}", magnet_link.tracker().unwrap_or("N/A"));

    let resp = utils::get_response(&magnet_link).await?;

    for peer in resp.peers {
        let info_hash = magnet_link.info_hash();
        let peer_id = Bytes20::new(*b"-CT0001-012345678901");

        let mut stream = peer.connect(info_hash, peer_id).await?;

        let ext_id = stream
            .extension_handshake()
            .await?
            .metadata_ext_id()
            .ok_or("Peer did not advertise ut_metadata extension")?;

        stream
            .send_message(Extension::RequestMetadata { ext_id, piece: 0 })
            .await?;

        let info = match stream.wait_extention().await? {
            Extension::Metadata { data, .. } => {
                let mut deserializer = Deserializer::new(data.as_ref());
                Info::deserialize(&mut deserializer)?
            }
            _ => return Err("Unexpected extension message".into()),
        };

        utils::print_info(&info)?;
    }

    Ok(())
}
