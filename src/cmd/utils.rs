use crate::{
    Result,
    meta::{AsTrackerRequest, Info, Meta, TrackerResponse},
    net::{
        Piece,
        broker::{self, Broker},
    },
    util::{Bytes20, RotationPool},
};
use tokio::sync::mpsc::{self, Receiver};
use tracing::info;

pub fn print_info(info: &Info) -> Result<()> {
    println!("Length: {}", info.length);
    println!("Info Hash: {}", info.hash()?.hex_encoded());
    println!("Piece Length: {}", info.piece_length);
    println!("Piece Hashes:");

    for hash in info.piece_hashes() {
        println!("{}", hash.hex_encoded());
    }

    Ok(())
}

pub(crate) async fn get_response<R: AsTrackerRequest>(req: &R) -> Result<TrackerResponse> {
    let resp = req.as_tracker_request()?.send().await?;
    Ok(resp)
}

pub(crate) async fn get_brokers(meta: &Meta) -> Result<(RotationPool<Broker>, Receiver<Piece>)> {
    let info_hash = meta.info.hash()?;
    let peer_id = Bytes20::new(*b"-CT0001-012345678901");

    let resp = get_response(meta).await?;
    info!("Found {} peers", resp.peers.as_ref().len());

    for (i, peer) in resp.peers.as_ref().iter().enumerate() {
        info!("Peer {}: {peer}", i + 1);
    }

    let mut brokers: Vec<Broker> = Vec::with_capacity(resp.peers.as_ref().len());
    let mut rxs: Vec<Receiver<Piece>> = Vec::with_capacity(resp.peers.as_ref().len());

    for peer in resp.peers.as_ref() {
        let mut stream = peer.connect(info_hash, peer_id).await?;
        stream.ready().await?;

        let (b, piece_rx) = broker::create(stream);
        brokers.push(b);
        rxs.push(piece_rx);
    }

    let brokers = RotationPool::from_iter(brokers);
    let (merged_tx, merged_rx) = mpsc::channel::<Piece>(100);

    for mut rx in rxs {
        let tx = merged_tx.clone();

        tokio::spawn(async move {
            while let Some(piece) = rx.recv().await {
                if tx.send(piece).await.is_err() {
                    break;
                }
            }
        });
    }

    Ok((brokers, merged_rx))
}
