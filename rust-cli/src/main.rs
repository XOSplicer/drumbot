mod api;
mod audio;

use api::Pattern;
use env_logger;
use failure::Error;
use std::{
    thread,
    collections::HashMap
};

fn main() -> Result<(), Error> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::try_init()?;
    let base_url = &"https://api.noopschallenge.com/drumbot";
    let patterns: HashMap<String, Pattern> = api::fetch_all(base_url)?
        .into_iter()
        .map(|p| (p.name().to_owned(), p))
        .collect();
    println!("{:#?}", &patterns);
    audio::run()?;
    Ok(())
}
