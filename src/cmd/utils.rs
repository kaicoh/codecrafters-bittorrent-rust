use crate::{
    BitTorrentError, Result,
    bencode::Deserializer,
    meta::{AsTrackerRequest, Info, TrackerResponse},
    net::{
        Extension, Peer, PeerStream, Piece,
        broker::{self, Broker},
    },
    util::{Bytes20, RotationPool},
};
use serde::Deserialize;
use tokio::sync::mpsc::{self, Receiver};
use tracing::warn;

macro_rules! err {
    ($msg:expr) => {
        BitTorrentError::Other($msg.into())
    };
}

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

pub(crate) async fn connect(peers: &[Peer], info_hash: Bytes20) -> Result<Vec<PeerStream>> {
    let peer_id = Bytes20::new(*b"-CT0001-012345678901");
    let mut streams: Vec<PeerStream> = Vec::new();

    for peer in peers {
        match peer.connect(info_hash, peer_id).await {
            Ok(stream) => streams.push(stream),
            Err(err) => {
                warn!("Failed to connect to peer {peer}: {err}");
                continue;
            }
        }
    }

    Ok(streams)
}

pub(crate) async fn broker_channels<S>(
    streams: S,
) -> Result<(RotationPool<Broker>, Receiver<Piece>)>
where
    S: IntoIterator<Item = PeerStream>,
{
    let mut brokers: Vec<Broker> = Vec::new();
    let mut rxs: Vec<Receiver<Piece>> = Vec::new();

    for mut stream in streams {
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

pub(crate) async fn get_ext_info(streams: &mut [PeerStream]) -> Result<Info> {
    for stream in streams.iter_mut() {
        let ext_id = stream
            .extension_handshake()
            .await?
            .metadata_ext_id()
            .ok_or(err!("Peer did not advertise ut_metadata extension"))?;

        stream
            .send_message(crate::net::Extension::RequestMetadata { ext_id, piece: 0 })
            .await?;

        match stream.wait_extention().await? {
            Extension::Metadata { data, .. } => {
                let mut deserializer = Deserializer::new(data.as_ref());
                let info = Info::deserialize(&mut deserializer)?;
                return Ok(info);
            }
            _ => continue,
        };
    }

    Err(err!("Failed to retrieve metadata from any peer"))
}
