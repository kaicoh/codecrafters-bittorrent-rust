use crate::{Result, meta::MagnetLink};

use std::str::FromStr;

pub(crate) async fn run(url: String) -> Result<()> {
    let magnet_link = MagnetLink::from_str(&url)?;
    println!("Tracker URL: {}", magnet_link.tracker().unwrap_or("N/A"));
    println!("Info Hash: {}", magnet_link.info_hash().hex_encoded());
    Ok(())
}
