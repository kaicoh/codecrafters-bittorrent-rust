use crate::{
    Result,
    meta::{Meta, TrackerRequest, TrackerResponse},
    net::{Broker, Piece},
    util::{Bytes20, RotationPool},
};
use tokio::sync::mpsc::{self, Receiver};

pub(crate) async fn get_response(meta: &Meta) -> Result<TrackerResponse> {
    let resp = TrackerRequest::builder()
        .url(&meta.announce)
        .info_hash(meta.info.hash()?)
        .left(meta.info.length)
        .build()?
        .send()
        .await?;
    Ok(resp)
}

pub(crate) async fn get_brokers(meta: &Meta) -> Result<(RotationPool<Broker>, Receiver<Piece>)> {
    let info_hash = meta.info.hash()?;
    let peer_id = Bytes20::new(*b"-CT0001-012345678901");

    let resp = get_response(meta).await?;
    println!("Found {} peers", resp.peers.as_ref().len());

    for (i, peer) in resp.peers.as_ref().iter().enumerate() {
        println!("Peer {}: {peer}", i + 1);
    }

    let mut brokers: Vec<Broker> = Vec::with_capacity(resp.peers.as_ref().len());
    let mut rxs: Vec<Receiver<Piece>> = Vec::with_capacity(resp.peers.as_ref().len());

    for peer in resp.peers.as_ref() {
        let mut stream = peer.connect(info_hash, peer_id).await?;
        stream.ready().await?;

        let (broker, piece_rx) = Broker::new(stream);
        brokers.push(broker);
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
