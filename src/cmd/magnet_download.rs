use crate::{meta::MagnetLink, net::Piece};

use super::utils;
use std::error::Error;
use std::str::FromStr;
use tracing::debug;

pub(crate) async fn run(output: String, url: String) -> Result<(), Box<dyn Error>> {
    let magnet_link = MagnetLink::from_str(&url)?;

    let resp = utils::get_response(&magnet_link).await?;
    let peers = resp.peers.as_ref();

    let info_hash = magnet_link.info_hash();
    let mut streams = utils::connect(peers, info_hash).await?;

    let info = utils::get_ext_info(&mut streams).await?;

    let (mut brokers, mut piece_rx) = utils::broker_channels(streams).await?;

    let hashes = info.piece_hashes();
    let mut pieces: Vec<Piece> = Vec::with_capacity(hashes.len());

    for (index, _) in hashes.iter().enumerate() {
        let broker = brokers.get_item();
        let length = info.piece_length(index);
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
