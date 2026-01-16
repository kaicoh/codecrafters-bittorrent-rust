use crate::{meta::MagnetLink, util::Bytes20};

use super::utils;
use std::error::Error;
use std::str::FromStr;
use tracing::info;

pub(crate) async fn run(output: String, url: String, index: u32) -> Result<(), Box<dyn Error>> {
    let magnet_link = MagnetLink::from_str(&url)?;

    let resp = utils::get_response(&magnet_link).await?;
    let peers = resp.peers.as_ref();

    let info_hash = magnet_link.info_hash();
    let mut streams = utils::connect(peers, info_hash).await?;

    let info = utils::get_ext_info(&mut streams).await?;
    let length = info.piece_length(index as usize);
    let piece_hash = info
        .piece_hashes()
        .get(index as usize)
        .copied()
        .ok_or_else(|| format!("Invalid piece index: {index}"))?;

    info!("Downloading piece {index}...");

    let (mut brokers, mut piece_rx) = utils::broker_channels(streams).await?;
    let broker = brokers.get_item();
    broker.request_piece(index as usize, length).await;

    info!("Waiting for piece {index} data...");

    if let Some(piece) = piece_rx.recv().await {
        let hash = Bytes20::sha1_hash(&piece.data);

        if piece_hash == hash {
            std::fs::write(output, piece.data)?;

            info!("🎉 Piece {index} downloaded and verified.");

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
