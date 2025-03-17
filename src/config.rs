use toml;
use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub blob_storage: BlobStorageConfig,
}

#[derive(Deserialize, Clone)]
pub struct BlobStorageConfig {
    /// where cached files are located
    pub location: String,
}

impl Config {
    pub fn parse(config: Option<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(config.unwrap_or(String::from("portcache.toml")))?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
