use crate::{meta::Meta, net::Piece};

use super::utils;
use std::error::Error;
use tracing::debug;

pub(crate) async fn run(output: String, path: String) -> Result<(), Box<dyn Error>> {
    let meta = Meta::from_path(&path)?;
    let (mut brokers, mut piece_rx) = utils::get_brokers(&meta).await?;

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
