use rayon::prelude::*;
use reqwest;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct SlimPattern {
    name: String,
}

impl SlimPattern {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Pattern {
    name: String,
    #[serde(rename = "stepCount")]
    step_count: u32,
    #[serde(rename = "beatsPerMinute")]
    bpm: u32,
    tracks: Vec<Track>,
}

impl Pattern {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    pub fn step_count(&self) -> u32 {
        self.step_count
    }
    pub fn bpm(&self) -> u32 {
        self.bpm
    }
    pub fn tracks(&self) -> &[Track] {
        self.tracks.as_slice()
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Track {
    instrument: String,
    steps: Vec<i32>
}

impl Track {
    pub fn instrument(&self) -> &str {
        self.instrument.as_str()
    }
    pub fn steps(&self) -> &[i32] {
        self.steps.as_slice()
    }
}

pub fn fetch_patterns(base_url: &str) -> Result<Vec<SlimPattern>, reqwest::Error> {
    Ok(reqwest::get(&format!("{}/patterns", base_url))?.json()?)
}

pub fn fetch_pattern(base_url: &str, name: &str) -> Result<Pattern, reqwest::Error> {
    Ok(reqwest::get(&format!("{}/patterns/{}", base_url, name))?.json()?)
}

pub fn fetch_all(base_url: &str) -> Result<Vec<Pattern>, reqwest::Error> {
    fetch_patterns(base_url)?
        .par_iter()
        .map(|p| fetch_pattern(base_url, p.name()))
        .collect()
}