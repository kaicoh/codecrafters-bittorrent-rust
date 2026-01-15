use crate::{Result, meta::Meta};

pub(crate) async fn run(path: String) -> Result<()> {
    let meta = Meta::from_path(&path)?;
    println!("Tracker URL: {}", meta.announce);
    println!("Length: {}", meta.info.length);

    let info = meta.info;

    println!("Info Hash: {}", info.hash()?.hex_encoded());
    println!("Piece Length: {}", info.piece_length);
    println!("Piece Hashes:");

    for hash in info.piece_hashes() {
        println!("{}", hash.hex_encoded());
    }
    Ok(())
}
