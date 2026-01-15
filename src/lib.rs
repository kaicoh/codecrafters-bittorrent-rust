pub mod bencode;
pub(crate) mod cli;
mod cmd;
mod error;
pub mod meta;
pub mod net;
pub mod util;

pub use cli::{Cli, Command};
pub use error::BitTorrentError;

pub type Result<T> = std::result::Result<T, BitTorrentError>;
