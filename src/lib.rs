pub mod bencode;
mod cli;
mod error;
pub mod file;
pub mod tracker;
pub mod util;

pub use cli::{Cli, Command};
pub use error::BitTorrentError;

pub type Result<T> = std::result::Result<T, BitTorrentError>;
