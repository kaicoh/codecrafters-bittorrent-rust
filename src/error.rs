use thiserror::Error;

#[derive(Debug, Error)]
pub enum BitTorrentError {
    #[error("Bencode Error: {0}")]
    BencodeError(&'static str),

    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Parse Int Error: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error("UTF8 Error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("FromUtf8 Error: {0}")]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error("{0}")]
    SerdeError(String),
}
