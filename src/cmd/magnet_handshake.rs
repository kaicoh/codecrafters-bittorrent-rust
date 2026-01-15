use crate::{
    bencode::Bencode,
    meta::MagnetLink,
    net::{Extension, Message},
    util::Bytes20,
};

use super::utils;
use std::collections::HashMap;
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

        stream.wait_bitfield().await?;
        stream.send_message(msg_payload()).await?;

        let msg = stream.wait_message(is_ext_handshake).await?;

        match msg {
            Message::Extension(Extension::Handshake(dict)) => {
                let ext_id =
                    metadata_ext_id(&dict).ok_or("Peer did not advertise ut_metadata extension")?;
                println!("Peer Metadata Extension ID: {ext_id}");
            }
            _ => {
                return Err("Did not receive a valid extension handshake from peer".into());
            }
        }
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

fn is_ext_handshake(msg: &Message) -> bool {
    matches!(msg, Message::Extension(Extension::Handshake(_)))
}

fn metadata_ext_id(dict: &HashMap<String, Bencode>) -> Option<u8> {
    if let Some(Bencode::Dict(m)) = dict.get("m")
        && let Some(Bencode::Int(id)) = m.get("ut_metadata")
    {
        Some(*id as u8)
    } else {
        None
    }
}
