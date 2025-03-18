use tokio::{fs, io::{self, AsyncWriteExt}};
use futures_core::stream::Stream;
use futures::stream::StreamExt;

use crate::utils;
use crate::fetcher::Fetcher;
use crate::config;

/// storage for downloaded blobs
#[derive(Clone)]
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
        let fetcher = Fetcher::new(&config).await?;
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

    /// store a blob from a stream in the storage
    /// @param name  name of the blob
    /// @param blob  a bytes stream with the blob
    pub async fn store(&self, name: String,
        blob: &mut (impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + std::marker::Unpin))
        -> Result<(), Box<dyn std::error::Error>> {
        let path = format!(
            "{}/{}/{}",
            self.location.clone(),
            utils::filename_hash_dir_blake2b(name.clone())?,
            name.clone(),
        );
        let path = std::path::Path::new(&path);

        // file exists - for now just exit
        // although best case we don't even attempt to re-download
        if path.is_file() {
            return Ok(())
        }

        // create dir for this blob if needed
        // assert that parent is not / or empty
        assert!(path.parent().is_some());
        if !path.parent().unwrap().is_dir() {
            fs::create_dir(path.parent().unwrap()).await?;
        }

        // write file chunks
        let file = fs::File::create(path).await?;
        let mut writer = io::BufWriter::new(file);

        while let Some(chunk) = blob.next().await {
            if chunk.is_err() {
                writer.flush().await?;
                eprintln!("Error while downloading {}: {}", name.clone(), chunk.err().unwrap().to_string());
                fs::remove_file(path).await?;
                return Err("Download failed".into());
            }

            writer.write(&chunk?).await?;
        }

        writer.flush().await?;

        Ok(())
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
    pub async fn request(&mut self, digest: String, file: String) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        // where we expect the file in storage
        let path = format!("{}/{}/{}",
                self.location.clone(),
                digest.clone(),
                file.clone()
        );
        let path = std::path::Path::new(&path);

        // first check if it's already there
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
        
        // then ask fetcher
        self.fetcher.fetch(file.clone(), self.clone()).await?;
        if path.is_file() {
            return Ok(path.to_path_buf());
        }

        Err(format!("Could not obtain file {}", file.clone()).into())
    }
}
