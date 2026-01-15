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

    #[error("Serde Error: {0}")]
    SerdeError(String),

    #[error("Deserialization Error: {0}")]
    DeserdeError(String),

    #[error("UrlEncodeError: {0}")]
    UrlEncodeError(#[from] serde_urlencoded::ser::Error),

    #[error("TrackerError: {0}")]
    TrackerError(&'static str),

    #[error("Reqwest Error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Url parse Error: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Address parse Error: {0}")]
    AddrParseError(#[from] std::net::AddrParseError),

    #[error("Invalid peer message: {0}")]
    InvalidPeerMessage(String),

    #[error("Connection closed unexpectedly")]
    ConnectionClosed,

    #[error("Unexpected channel closed")]
    ChannelClosed,
}
