use crate::{meta::Meta, util::Bytes20};

use super::utils;
use std::error::Error;
use tracing::info;

pub(crate) async fn run(output: String, path: String, index: u32) -> Result<(), Box<dyn Error>> {
    let meta = Meta::from_path(&path)?;
    let info_hash = meta.info.hash()?;

    let resp = utils::get_response(&meta).await?;
    let peers = resp.peers.as_ref();

    let (mut brokers, mut piece_rx) = utils::broker_channels(peers, info_hash).await?;

    let length = meta.piece_length(index as usize);

    let piece_hash = meta
        .info
        .piece_hashes()
        .get(index as usize)
        .copied()
        .ok_or_else(|| format!("Invalid piece index: {index}"))?;

    info!("Downloading piece {index}...");

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
