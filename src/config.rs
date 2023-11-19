use std::fs;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
}

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    // Load configuration from environment variables or a configuration file

    let config_str = fs::read_to_string("Config.toml")?;

    let config: Config = toml::from_str(&config_str)?;
    Ok(config)
}