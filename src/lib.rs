pub mod bencode;
mod cli;
mod error;
pub mod file;

pub use cli::{Cli, Command};
pub use error::BitTorrentError;

pub type Result<T> = std::result::Result<T, BitTorrentError>;
