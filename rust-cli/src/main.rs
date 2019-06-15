mod api;
mod audio;

use api::Pattern;
use env_logger;
use failure::Error;
use std::{
    thread,
    collections::HashMap,
    time::Duration,
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
    let mut engine = audio::run()?;

    let t2 = thread::spawn(move || {
        loop {
            // FIXME: unwrap
            engine.dispatch_wav("res/samples/kick.wav").unwrap();
            thread::sleep(Duration::from_millis(250));
            engine.dispatch_wav("res/samples/kick.wav").unwrap();
            engine.dispatch_wav("res/samples/cowbell.wav").unwrap();
            thread::sleep(Duration::from_millis(250));
            engine.dispatch_wav("res/samples/kick.wav").unwrap();
            thread::sleep(Duration::from_millis(250));
            engine.dispatch_wav("res/samples/kick.wav").unwrap();
            engine.dispatch_wav("res/samples/hihat.wav").unwrap();
            thread::sleep(Duration::from_millis(250));
        }
    });

    t2.join().unwrap();

    Ok(())
}
