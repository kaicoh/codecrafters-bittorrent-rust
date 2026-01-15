use codecrafters_bittorrent as bit;

use bit::{
    Cli, Command,
    bencode::Bencode,
    meta::{Meta, TrackerRequest, TrackerResponse},
    net::{Broker, Peer, Piece},
    util::{Bytes20, RotationPool},
};
use clap::Parser;
use std::error::Error;
use std::str::FromStr;
use tokio::sync::mpsc::{self, Receiver};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Decode { token } => {
            let v = Bencode::parse(token.as_bytes())?;
            println!("{v}");
        }
        Command::Info { path } => {
            let meta = Meta::from_path(&path)?;
            println!("Tracker URL: {}", meta.announce);
            println!("Length: {}", meta.info.length);

            let info = meta.info;

            println!("Info Hash: {}", info.hash()?.hex_encoded());
            println!("Piece Length: {}", info.piece_length);
            println!("Piece Hashes:");

            for hash in info.piece_hashes() {
                println!("{}", hash.hex_encoded());
            }
        }
        Command::Peers { path } => {
            let meta = Meta::from_path(&path)?;
            let resp = get_tracker_response(&meta).await?;

            for peer in resp.peers {
                println!("{peer}");
            }
        }
        Command::Handshake { path, address } => {
            let meta = Meta::from_path(&path)?;
            let info_hash = meta.info.hash()?;
            let peer_id = Bytes20::new(*b"-CT0001-012345678901");

            let stream = Peer::from_str(&address)?
                .connect(info_hash, peer_id)
                .await?;
            println!("Peer ID: {}", stream.peer_id().hex_encoded());
        }
        Command::DownloadPiece {
            output,
            path,
            index,
        } => {
            let meta = Meta::from_path(&path)?;
            let (mut brokers, mut piece_rx) = get_brokers(&meta).await?;

            let length = meta.piece_length(index as usize);

            let piece_hash = meta
                .info
                .piece_hashes()
                .get(index as usize)
                .copied()
                .ok_or_else(|| format!("Invalid piece index: {index}"))?;

            println!("Downloading piece {index}...");

            let broker = brokers.get_item();
            broker.request_piece(index as usize, length).await;

            println!("Waiting for piece {index} data...");
            if let Some(piece) = piece_rx.recv().await {
                let hash = Bytes20::sha1_hash(&piece.data);

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

            return Err("Failed to receive piece data".into());
        }
        Command::Download { output, path } => {
            let meta = Meta::from_path(&path)?;
            let (mut brokers, mut piece_rx) = get_brokers(&meta).await?;

            let hashes = meta.piece_hashes();
            let mut pieces: Vec<Piece> = Vec::with_capacity(hashes.len());

            for (index, _) in hashes.iter().enumerate() {
                let broker = brokers.get_item();
                let length = meta.piece_length(index);
                broker.request_piece(index, length).await;
            }

            while let Some(piece) = piece_rx.recv().await {
                pieces.push(piece);
                println!("Downloaded piece {}/{}", pieces.len(), hashes.len());

                if pieces.len() == hashes.len() {
                    break;
                }
            }

            pieces.sort_by_key(|p| p.index);

            let file_data = pieces.into_iter().flat_map(|d| d.data).collect::<Vec<u8>>();

            std::fs::write(output, file_data)?;
        }
    }

    Ok(())
}

async fn get_tracker_response(meta: &Meta) -> Result<TrackerResponse, Box<dyn Error>> {
    let resp = TrackerRequest::builder()
        .url(&meta.announce)
        .info_hash(meta.info.hash()?)
        .left(meta.info.length)
        .build()?
        .send()
        .await?;
    Ok(resp)
}

async fn get_brokers(
    meta: &Meta,
) -> Result<(RotationPool<Broker>, Receiver<Piece>), Box<dyn Error>> {
    let info_hash = meta.info.hash()?;
    let peer_id = Bytes20::new(*b"-CT0001-012345678901");

    let resp = get_tracker_response(meta).await?;
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
