use crate::meta::MagnetLink;

use super::utils;
use std::error::Error;
use std::str::FromStr;

pub(crate) async fn run(url: String) -> Result<(), Box<dyn Error>> {
    let magnet_link = MagnetLink::from_str(&url)?;
    println!("Tracker URL: {}", magnet_link.tracker().unwrap_or("N/A"));

    let resp = utils::get_response(&magnet_link).await?;
    let peers = resp.peers.as_ref();

    let info_hash = magnet_link.info_hash();
    let mut streams = utils::connect(peers, info_hash).await?;

    let info = utils::get_ext_info(&mut streams).await?;
    utils::print_info(&info)?;

    Ok(())
}
