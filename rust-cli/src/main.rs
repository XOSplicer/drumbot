use failure::Error;
use std::thread;
mod api;
mod audio;


fn main() -> Result<(), Error> {
    let base_url = &"https://api.noopschallenge.com/drumbot";
    let patterns = api::fetch_all(base_url)?;
    println!("{:#?}", &patterns);
    let t = thread::spawn(move || {
        audio::run();
    });
    t.join().expect("Could not join thread");
    Ok(())
}
