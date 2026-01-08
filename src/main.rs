use codecrafters_bittorrent as bit;

use bit::{
    Cli, Command,
    bencode::Bencode,
    meta::{Meta, TrackerRequest, TrackerResponse},
    peers::{Download, Peer},
    util::{Bytes20, Pool},
};
use clap::Parser;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

const MAX_ATTEMPTS: u8 = 5;

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

            let conn = Peer::from_str(&address)?.connect(info_hash, peer_id)?;
            println!("Peer ID: {}", conn.peer_id().hex_encoded());
        }
        Command::DownloadPiece {
            output,
            path,
            index,
        } => {
            let meta = Meta::from_path(&path)?;
            let info_hash = meta.info.hash()?;
            let peer_id = Bytes20::new(*b"-CT0001-012345678901");

            let resp = get_tracker_response(&meta).await?;
            println!("Found {} peers", resp.peers.as_ref().len());

            for (i, peer) in resp.peers.as_ref().iter().enumerate() {
                println!("Peer {}: {peer}", i + 1);
            }

            let mut pool = Pool::from_iter(resp.peers);

            let length = meta.piece_length(index as usize);
            let piece_hash = meta
                .info
                .piece_hashes()
                .get(index as usize)
                .copied()
                .ok_or_else(|| format!("Invalid piece index: {index}"))?;
            println!(
                "Expected hash for piece {index}: {}",
                piece_hash.hex_encoded()
            );

            let mut attempts = 0;

            while attempts < MAX_ATTEMPTS {
                let peer = pool.get_item().await;
                let mut conn = peer.connect(info_hash, peer_id)?;

                conn.ready()?;

                let piece_data = conn.download_piece(index, length as u32).await?;
                let hash = Bytes20::sha1_hash(&piece_data);
                println!(
                    "Downloaded piece {index} from Peer: {peer}. Length: {}. Hash: {}",
                    piece_data.len(),
                    hash.hex_encoded()
                );

                if piece_hash == hash {
                    std::fs::write(output, piece_data)?;
                    println!("🎉 Piece {index} downloaded and verified.");
                    break;
                } else {
                    attempts += 1;
                    println!("Hash mismatch for piece {index}. Attempt {attempts}/{MAX_ATTEMPTS}.",);
                }
            }

            if attempts == MAX_ATTEMPTS {
                return Err(format!(
                    "Failed to download piece {index} after {MAX_ATTEMPTS} attempts"
                )
                .into());
            }
        }
        Command::Download { output, path } => {
            let meta = Meta::from_path(&path)?;
            let info_hash = meta.info.hash()?;
            let peer_id = Bytes20::new(*b"-CT0001-012345678901");

            let resp = get_tracker_response(&meta).await?;
            println!("Found {} peers", resp.peers.as_ref().len());

            for (i, peer) in resp.peers.as_ref().iter().enumerate() {
                println!("Peer {}: {peer}", i + 1);
            }

            let pool = Arc::new(Mutex::new(Pool::from_iter(resp.peers)));

            println!("Piece Length: {}", meta.info.piece_length);
            let hashes = meta.piece_hashes();
            for h in hashes {
                println!("Piece hash: {}", h.hex_encoded());
            }

            let num_pieces = hashes.len();

            let mut downloads: Vec<Download> = Vec::new();
            let mut tasks = tokio::task::JoinSet::<Download>::new();

            for (index, h) in hashes.iter().enumerate() {
                let mut attempts = 0;
                let h = *h;

                let length = meta.piece_length(index);
                let pool = Arc::clone(&pool);

                tasks.spawn(async move {
                    while attempts < MAX_ATTEMPTS {
                        let peer = {
                            let mut pool = pool.lock().await;
                            pool.get_item().await
                        };

                        let mut conn = peer
                            .connect(info_hash, peer_id)
                            .expect("Failed to connect to peer");

                        conn.ready().expect("Failed to ready connection");

                        let piece_data = conn
                            .download_piece(index as u32, length as u32)
                            .await
                            .expect("Failed to download piece");

                        println!(
                            "Downloaded piece {}/{} from Peer: {peer}. Length: {}",
                            index + 1,
                            num_pieces,
                            piece_data.len()
                        );

                        let hash = Bytes20::sha1_hash(&piece_data);

                        if h == hash {
                            println!(
                                "🎉 Downloaded and verified piece {}/{num_pieces}",
                                index + 1
                            );

                            return Download {
                                index: index as u32,
                                block: piece_data,
                            };
                        } else {
                            attempts += 1;

                            println!(
                                "🤔 Hash mismatch for piece {}. Expected {}, got {}. Retrying {attempts}/{MAX_ATTEMPTS}",
                                index + 1,
                                h.hex_encoded(),
                                hash.hex_encoded()
                            );
                        }
                    }

                    panic!(
                        "Failed to download piece {} after {MAX_ATTEMPTS} attempts",
                        index + 1
                    );
                });
            }

            while let Some(res) = tasks.join_next().await {
                let download = res?;
                downloads.push(download);
            }

            downloads.sort();

            let file_data = downloads
                .into_iter()
                .flat_map(|d| d.block)
                .collect::<Vec<u8>>();

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
