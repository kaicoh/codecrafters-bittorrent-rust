use crate::cmd;

use clap::{Parser, Subcommand};
use std::error::Error;

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
#[command(rename_all = "snake_case")]
pub enum Command {
    Decode {
        token: String,
    },
    Info {
        path: String,
    },
    Peers {
        path: String,
    },
    Handshake {
        path: String,
        address: String,
    },
    DownloadPiece {
        #[arg(short, long)]
        output: String,
        path: String,
        index: u32,
    },
    Download {
        #[arg(short, long)]
        output: String,
        path: String,
    },
    MagnetParse {
        uri: String,
    },
    MagnetHandshake {
        uri: String,
    },
}

impl Command {
    pub async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Decode { token } => cmd::decode::run(token).await?,
            Self::Info { path } => cmd::info::run(path).await?,
            Self::Peers { path } => cmd::peers::run(path).await?,
            Self::Handshake { path, address } => cmd::handshake::run(path, address).await?,
            Self::DownloadPiece {
                output,
                path,
                index,
            } => cmd::download_piece::run(output, path, index).await?,
            Self::Download { output, path } => cmd::download::run(output, path).await?,
            Self::MagnetParse { uri } => cmd::magnet_parse::run(uri).await?,
            Self::MagnetHandshake { uri } => cmd::magnet_handshake::run(uri).await?,
        }

        Ok(())
    }
}
