use crate::{meta::Meta, net::Piece};

use super::utils;
use std::error::Error;
use tracing::debug;

pub(crate) async fn run(output: String, path: String) -> Result<(), Box<dyn Error>> {
    let meta = Meta::from_path(&path)?;
    let info_hash = meta.info.hash()?;

    let resp = utils::get_response(&meta).await?;
    let peers = resp.peers.as_ref();

    let (mut brokers, mut piece_rx) = utils::broker_channels(peers, info_hash).await?;

    let hashes = meta.piece_hashes();
    let mut pieces: Vec<Piece> = Vec::with_capacity(hashes.len());

    for (index, _) in hashes.iter().enumerate() {
        let broker = brokers.get_item();
        let length = meta.piece_length(index);
        broker.request_piece(index, length).await;
    }

    while let Some(piece) = piece_rx.recv().await {
        pieces.push(piece);
        debug!("Downloaded piece {}/{}", pieces.len(), hashes.len());

        if pieces.len() == hashes.len() {
            break;
        }
    }

    pieces.sort_by_key(|p| p.index);

    let file_data = pieces.into_iter().flat_map(|d| d.data).collect::<Vec<u8>>();

    std::fs::write(output, file_data)?;

    Ok(())
}
