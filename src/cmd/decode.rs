use crate::{Result, bencode::Bencode};

pub(crate) async fn run(token: String) -> Result<()> {
    let v = Bencode::parse(token.as_bytes())?;
    println!("{v}");
    Ok(())
}
