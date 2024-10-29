use std::fs;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub listener: String,
}

pub fn parse_config(conf: &str) -> Config {
    let contents = fs::read_to_string(conf).expect("Unable to read config file");
    let config: Config = toml::from_str(&contents).expect("Unable to parse config file");
    config
}
