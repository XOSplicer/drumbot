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
    // println!("{:#?}", &patterns);

    let instrument_lookup = ["cowbell", "hihat", "kick", "ride", "clap", "rim", "snare"]
        .into_iter()
        .map(|s| (*s, format!("res/samples/{}.wav", &s)))
        .chain(std::iter::once(("clap", "/res/samples/hihat.wav".into())))
        .collect::<HashMap<&'static str, String>>();

    let p = patterns.values().skip(0).next().unwrap().clone();
    let delay = Duration::from_millis(
        (60.0 * 1_000.0 / (p.bpm() as f32)) as u64
    ); //[sec / minute] * [millis / sec] / [Beats / Minute]

    println!("Selected pattern: {}", p.name());
    println!("BPM {} -> delay {:?}", p.bpm(), &delay);
    println!("{:#?}", &p);


    let mut engine = audio::run()?;

    let t2 = thread::spawn(move || {
        loop {
            for step in 0..(p.step_count() as usize) {
                for track in p.tracks() {
                    let active = track.steps()[step] != 0;
                    println!("Playing step {} instrument {} active {}", &step, &track.instrument(), &active);
                    if active {
                        if let Some(path) = instrument_lookup.get(track.instrument()) {
                            engine.dispatch_wav(path).unwrap();
                        }
                    }
                }
                thread::sleep(delay);
            }
        }
    });

    t2.join().unwrap();

    Ok(())
}
