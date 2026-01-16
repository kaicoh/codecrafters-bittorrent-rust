use crate::{
    Result,
    meta::{AsTrackerRequest, Info, TrackerResponse},
    net::{
        Peer, Piece,
        broker::{self, Broker},
    },
    util::{Bytes20, RotationPool},
};
use tokio::sync::mpsc::{self, Receiver};

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

pub(crate) async fn broker_channels<'a>(
    peers: &'a [Peer],
    info_hash: Bytes20,
) -> Result<(RotationPool<Broker>, Receiver<Piece>)> {
    let peer_id = Bytes20::new(*b"-CT0001-012345678901");

    let mut brokers: Vec<Broker> = Vec::with_capacity(peers.len());
    let mut rxs: Vec<Receiver<Piece>> = Vec::with_capacity(peers.len());

    for peer in peers {
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
