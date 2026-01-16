use crate::{Result, meta::Meta};

use super::utils;

pub(crate) async fn run(path: String) -> Result<()> {
    let meta = Meta::from_path(&path)?;
    println!("Tracker URL: {}", meta.announce);

    utils::print_info(&meta.info)
}
