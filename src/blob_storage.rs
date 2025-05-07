use tokio::fs;

use crate::config;
use crate::fetcher::Fetcher;
use crate::utils;

/// storage for downloaded blobs
pub struct BlobStorage {
    /// root of the blob storage
    location: String,

    /// Fetcher used for fetching missing files
    fetcher: Fetcher,
}

impl BlobStorage {
    /// initialize a new blob storage directory structure
    /// if it doesn't already
    async fn init(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir(self.location.clone()).await?;
        Ok(())
    }

    /// create BlobStorage and setup fetcher for missing files
    /// @param config    Config struct
    /// @param location  root of the blob storage
    pub async fn new(config: &config::Config) -> Result<Self, Box<dyn std::error::Error>> {
        let fetcher = Fetcher::new(config).await?;
        let new = Self {
            location: config.storage.location.clone(),
            fetcher,
        };

        if !fs::try_exists(new.location.clone()).await? {
            println!("Initializing blob storage at {}", new.location.clone());
            new.init().await?;
        }

        Ok(new)
    }

    /// get storage location for a blob
    /// @param name  Name of the blob
    pub async fn blob_location(&self, name: &String) -> Result<std::path::PathBuf, String> {
        let path = format!(
            "{}/{}/{}",
            &self.location,
            utils::filename_hash_dir_blake2b(name.clone()).map_err(|x| x.to_string())?,
            &name
        );
        Ok(std::path::PathBuf::from(&path))
    }

    /// get an AsyncReader to the requested file
    /// if the file isn't cached we will request the fetcher to fetch it
    /// TODO: verify request digest matches filename
    /// TODO: _should_ be thread safe since for now since BlobStorage
    ///       is locked with a mutex from the rocket side
    /// TODO: should probably split this to a try_request() which does a read only
    ///       lookup in the cache and can serve without locking
    /// @param digest  2 byte blake2b512 digest of the filename
    /// @param file    file name
    pub async fn request(
        &self,
        file: &String,
    ) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        // where we expect the file in storage
        let path = self.blob_location(file).await?;

        // first check if it's already there
        if path.is_file() {
            return Ok(path.to_path_buf());
        }

        // then ask fetcher
        self.fetcher.fetch(file, self).await?;
        if path.is_file() {
            return Ok(path.to_path_buf());
        }

        Err(format!("Could not obtain file {}", file).into())
    }
}
