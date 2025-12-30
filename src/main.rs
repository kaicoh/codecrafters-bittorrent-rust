use codecrafters_bittorrent as bit;

use bit::{
    Cli, Command,
    bencode::{Bencode, Serializer},
    meta::{Meta, TrackerRequest, TrackerResponse},
    peers::{Download, Peer},
    util::{Bytes20, Pool},
};
use clap::Parser;
use serde::Serialize;
use sha1::{Digest, Sha1};
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
            let meta = get_meta(&path)?;
            println!("Tracker URL: {}", meta.announce);
            println!("Length: {}", meta.info.length);

            let info_hash = get_info_hash(&meta)?;
            let info = meta.info;

            println!("Info Hash: {}", info_hash.hex_encoded());
            println!("Piece Length: {}", info.piece_length);
            println!("Piece Hashes:");

            for hash in info.piece_hashes()? {
                println!("{}", hash.hex_encoded());
            }
        }
        Command::Peers { path } => {
            let meta = get_meta(&path)?;
            let info_hash = get_info_hash(&meta)?;

            let resp = get_tracker_response(&info_hash, &meta).await?;

            for peer in resp.peers {
                println!("{peer}");
            }
        }
        Command::Handshake { path, address } => {
            let meta = get_meta(&path)?;
            let info_hash = get_info_hash(&meta)?;
            let peer_id = Bytes20::new(*b"-CT0001-012345678901");

            let conn = Peer::from_str(&address)?.connect(info_hash, peer_id)?;
            println!("Peer ID: {}", conn.peer_id().hex_encoded());
        }
        Command::DownloadPiece {
            output,
            path,
            index,
        } => {
            let meta = get_meta(&path)?;
            let info_hash = get_info_hash(&meta)?;
            let peer_id = Bytes20::new(*b"-CT0001-012345678901");

            let resp = get_tracker_response(&info_hash, &meta).await?;
            let mut pool = Pool::from_iter(resp.peers);

            let length = get_piece_length(index, &meta)?;
            let piece_hash = meta
                .info
                .piece_hashes()?
                .get(index as usize)
                .copied()
                .ok_or("Invalid piece index")?;

            let mut attempts = 0;

            while attempts < MAX_ATTEMPTS {
                let peer = pool.get_item().await;
                let mut conn = peer.connect(info_hash, peer_id)?;

                conn.ready()?;

                let piece_data = conn.download_piece(index, length as u32).await?;

                let hash = sha1_hash(&piece_data);

                if piece_hash == hash {
                    std::fs::write(output, piece_data)?;
                    println!("🎉 Piece {index} downloaded and verified.");
                    break;
                } else {
                    attempts += 1;
                    println!("Hash mismatch for piece {index}. Attempt {attempts}/{MAX_ATTEMPTS}.",);
                }
            }
        }
        Command::Download { output, path } => {
            let meta = get_meta(&path)?;
            let info_hash = get_info_hash(&meta)?;
            let peer_id = Bytes20::new(*b"-CT0001-012345678901");

            let resp = get_tracker_response(&info_hash, &meta).await?;
            println!("Found {} peers", resp.peers.len());

            for (i, peer) in resp.peers.iter().enumerate() {
                println!("Peer {}: {peer}", i + 1);
            }

            let pool = Arc::new(Mutex::new(Pool::from_iter(resp.peers)));

            println!("Piece Length: {}", meta.info.piece_length);
            let hashes = meta.info.piece_hashes()?;
            for h in &hashes {
                println!("Piece hash: {}", h.hex_encoded());
            }

            let num_pieces = hashes.len();

            let mut downloads: Vec<Download> = Vec::new();
            let mut tasks = tokio::task::JoinSet::<Download>::new();

            for (index, h) in hashes.into_iter().enumerate() {
                let mut attempts = 0;

                let length = get_piece_length(index as u32, &meta)?;
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

                        let hash = sha1_hash(&piece_data);

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

fn get_meta(path: &str) -> Result<Meta, Box<dyn Error>> {
    let encoded = Bencode::from_path(path)?;
    let meta_info = Meta::try_from(&encoded)?;
    Ok(meta_info)
}

fn get_info_hash(meta: &Meta) -> Result<Bytes20, Box<dyn Error>> {
    let mut bytes = Vec::new();
    meta.info.serialize(&mut Serializer::new(&mut bytes))?;
    let info_hash = Bytes20::from(Sha1::digest(&bytes).as_ref());
    Ok(info_hash)
}

async fn get_tracker_response(
    hash: &Bytes20,
    meta: &Meta,
) -> Result<TrackerResponse, Box<dyn Error>> {
    let resp = TrackerRequest::builder()
        .url(&meta.announce)
        .info_hash(hash)
        .left(meta.info.length)
        .build()?
        .send()
        .await?;
    Ok(resp)
}

fn get_piece_length(index: u32, meta: &Meta) -> Result<u32, Box<dyn Error>> {
    let piece_length = meta.info.piece_length;
    let last_piece_length = (meta.info.length % piece_length as u64) as usize;
    let is_last_piece = (index as usize) == (meta.info.num_pieces()? - 1);

    let length = if is_last_piece {
        last_piece_length
    } else {
        piece_length as usize
    };
    Ok(length as u32)
}

fn sha1_hash(bytes: &[u8]) -> Bytes20 {
    let digest = Sha1::digest(bytes);
    Bytes20::from(digest.as_ref())
}
