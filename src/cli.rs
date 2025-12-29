use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Decode { token: String },
    Info { path: String },
    Peers { path: String },
    Handshake { path: String, address: String },
}
