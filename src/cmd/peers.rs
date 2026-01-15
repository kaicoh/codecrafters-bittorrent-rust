use super::utils;
use crate::{Result, meta::Meta};

pub(crate) async fn run(path: String) -> Result<()> {
    let meta = Meta::from_path(&path)?;
    let resp = utils::get_response(&meta).await?;

    for peer in resp.peers {
        println!("{peer}");
    }

    Ok(())
}
