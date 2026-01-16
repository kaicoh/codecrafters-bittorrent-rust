use crate::{
    bencode::Deserializer,
    meta::{Info, MagnetLink},
    net::{Extension, broker},
    util::Bytes20,
};

use super::utils;
use serde::Deserialize;
use std::error::Error;
use std::str::FromStr;

pub(crate) async fn run(output: String, url: String, index: u32) -> Result<(), Box<dyn Error>> {
    let magnet_link = MagnetLink::from_str(&url)?;

    let resp = utils::get_response(&magnet_link).await?;

    let peer = resp.peers.into_iter().next().ok_or("No peers found")?;

    let info_hash = magnet_link.info_hash();
    let peer_id = Bytes20::new(*b"-CT0001-012345678901");

    let mut stream = peer.connect(info_hash, peer_id).await?;

    let ext_id = stream
        .extension_handshake()
        .await?
        .metadata_ext_id()
        .ok_or("Peer did not advertise ut_metadata extension")?;

    stream
        .send_message(Extension::RequestMetadata {
            ext_id,
            piece: index,
        })
        .await?;

    let info = match stream.wait_extention().await? {
        Extension::Metadata { data, .. } => {
            let mut deserializer = Deserializer::new(data.as_ref());
            Info::deserialize(&mut deserializer)?
        }
        _ => return Err("Unexpected extension message".into()),
    };

    stream.send_interested().await?;
    stream.wait_unchoke().await?;

    let (mut b, mut piece_rx) = broker::create(stream);
    let length = info.piece_length(index as usize);
    b.request_piece(index as usize, length).await;

    if let Some(piece) = piece_rx.recv().await {
        let hash = Bytes20::sha1_hash(&piece.data);

        let piece_hash = info
            .piece_hashes()
            .get(index as usize)
            .copied()
            .ok_or_else(|| format!("Invalid piece index: {index}"))?;

        if piece_hash == hash {
            std::fs::write(output, piece.data)?;

            println!("🎉 Piece {index} downloaded and verified.");

            return Ok(());
        } else {
            return Err(format!(
                "Hash mismatch for piece {index}. Expected {}, got {}.",
                piece_hash.hex_encoded(),
                hash.hex_encoded()
            )
            .into());
        }
    }

    Err("Failed to receive piece data".into())
}
