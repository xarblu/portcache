use serde::Deserialize;
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub storage: BlobStorageConfig,
    pub fetcher: FetcherConfig,
    pub server: ServerConfig,
    pub repo: RepoConfig,
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

#[derive(Deserialize, Clone)]
pub struct ServerConfig {
    /// address rocket should listen on
    pub address: IpAddr,

    /// port to listen on
    pub port: u16,
}

#[derive(Deserialize, Clone)]
pub struct RepoConfig {
    /// interval in which to sync repos in minutes
    pub sync_interval: u64,

    /// path where the synced repos should be stored
    pub storage_root: PathBuf,

    /// list of repo urls to clone
    pub repos: Vec<String>,
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
