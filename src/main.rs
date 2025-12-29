use codecrafters_bittorrent as bit;

use bit::{
    Cli, Command,
    bencode::{Bencode, Serializer},
    peer::Peer,
    tracker::TrackerRequest,
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
            let encoded = Bencode::from_path(path)?;
            let meta_info = bit::file::MetaInfo::try_from(&encoded)?;
            println!("Tracker URL: {}", meta_info.announce);
            println!("Length: {}", meta_info.info.length);

            let info = meta_info.info;

            let mut bytes = Vec::new();
            info.serialize(&mut Serializer::new(&mut bytes))?;
            let hash = Sha1::digest(&bytes);
            println!("Info Hash: {:x}", hash);

            println!("Piece Length: {}", info.piece_length);

            println!("Piece Hashes:");
            for hash in info.piece_hashes()? {
                println!("{}", hash.hex_encoded());
            }
        }
        Command::Peers { path } => {
            let encoded = Bencode::from_path(path)?;
            let meta_info = bit::file::MetaInfo::try_from(&encoded)?;

            let mut bytes = Vec::new();
            meta_info.info.serialize(&mut Serializer::new(&mut bytes))?;
            let info_hash = Sha1::digest(&bytes);

            let resp = TrackerRequest::builder()
                .url(meta_info.announce)
                .info_hash(info_hash)
                .left(meta_info.info.length)
                .build()?
                .send()
                .await?;

            for peer in resp.peers {
                println!("{peer}");
            }
        }
        Command::Handshake { path, address } => {
            let encoded = Bencode::from_path(path)?;
            let meta_info = bit::file::MetaInfo::try_from(&encoded)?;

            let mut bytes = Vec::new();
            meta_info.info.serialize(&mut Serializer::new(&mut bytes))?;
            let info_hash = Bytes20::from(Sha1::digest(&bytes).as_ref());
            let peer_id = Bytes20::new(*b"-CT0001-012345678901");

            let resp = Peer::from_str(&address)?.handshake(info_hash, peer_id)?;
            println!("Peer ID: {}", resp.peer_id().hex_encoded());
        }
    }

    Ok(())
}
