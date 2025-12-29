use codecrafters_bittorrent as bit;

use bit::{
    Cli, Command,
    bencode::{Bencode, Serializer},
    meta::{Meta, TrackerRequest, TrackerResponse},
    peers::Peer,
    util::Bytes20,
};
use clap::Parser;
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::error::Error;
use std::str::FromStr;

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
            let peer = resp.peers.first().ok_or("No peers found")?;
            let mut conn = peer.connect(info_hash, peer_id)?;

            conn.wait_for_bitfield()?;
            conn.send_interested()?;
            conn.wait_for_unchoke()?;

            let length = get_piece_length(index, &meta)?;

            let piece_data = conn.download_piece(index, length as u32).await?;
            std::fs::write(output, piece_data)?;
        }
        Command::Download { output, path } => {
            let meta = get_meta(&path)?;
            let info_hash = get_info_hash(&meta)?;
            let peer_id = Bytes20::new(*b"-CT0001-012345678901");

            let resp = get_tracker_response(&info_hash, &meta).await?;
            let peer = resp.peers.first().ok_or("No peers found")?;
            let mut conn = peer.connect(info_hash, peer_id)?;

            conn.wait_for_bitfield()?;
            conn.send_interested()?;
            conn.wait_for_unchoke()?;

            let num_pieces = meta.info.num_pieces()?;
            let mut file_data = Vec::new();

            for index in 0..num_pieces {
                let length = get_piece_length(index as u32, &meta)?;
                let piece_data = conn.download_piece(index as u32, length as u32).await?;
                file_data.extend_from_slice(&piece_data);
                println!("Downloaded piece {}/{}", index + 1, num_pieces);
            }

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
