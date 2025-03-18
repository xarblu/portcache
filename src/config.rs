use toml;
use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub storage: BlobStorageConfig,
    pub fetcher: FetcherConfig,
}

#[derive(Deserialize, Clone)]
pub struct BlobStorageConfig {
    /// where cached files are located
    pub location: String,
}

#[derive(Deserialize, Clone)]
pub struct FetcherConfig {
    /// List of mirror urls
    /// Available mirrors: https://www.gentoo.org/downloads/mirrors/
    /// Currently only supports HTTP and HTTPS
    pub mirrors: Vec<String>,
}

impl Config {
    /// parse config file
    /// @param config  optional path to config file
    pub fn parse(config: Option<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(config.unwrap_or(String::from("portcache.toml")))?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
