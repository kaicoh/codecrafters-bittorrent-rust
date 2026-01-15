use crate::{Result, meta::Meta, net::Peer, util::Bytes20};
use std::str::FromStr;

pub(crate) async fn run(path: String, address: String) -> Result<()> {
    let meta = Meta::from_path(&path)?;
    let info_hash = meta.info.hash()?;
    let peer_id = Bytes20::new(*b"-CT0001-012345678901");

    let stream = Peer::from_str(&address)?
        .connect(info_hash, peer_id)
        .await?;

    println!("Peer ID: {}", stream.peer_id().hex_encoded());

    Ok(())
}
