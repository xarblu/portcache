use std::collections::HashMap;
use std::path::PathBuf;

use futures::lock::Mutex;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Notify;

use crate::config;
use crate::fetcher::Fetcher;
use crate::repo_db::RepoDB;
use crate::utils;

/// storage for downloaded blobs
pub struct BlobStorage {
    /// root of the blob storage
    location: PathBuf,

    /// Fetcher used for fetching missing files
    fetcher: Fetcher,

    /// tracker for Fetcher jobs
    /// maps file name to notifier to wait on
    fetch_jobs: Mutex<HashMap<String, Arc<Notify>>>,
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
    pub async fn new(
        config: &config::Config,
        repo_db: Arc<RepoDB>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let fetcher = Fetcher::new(config, repo_db.clone()).await?;
        let new = Self {
            location: config.storage.location.join("distfiles"),
            fetcher,
            fetch_jobs: Mutex::new(HashMap::new()),
        };

        if !new.location.exists() {
            println!(
                "Initializing blob storage at {}",
                new.location.to_string_lossy()
            );
            new.init().await?;
        }

        Ok(new)
    }

    /// get storage location for a blob
    /// @param name  Name of the blob
    pub async fn blob_location(&self, name: &String) -> Result<std::path::PathBuf, String> {
        let path = self
            .location
            .join(utils::filename_hash_dir_blake2b(name).map_err(|x| x.to_string())?)
            .join(name);
        Ok(std::path::PathBuf::from(&path))
    }

    /// get a PathBuf to the requested file
    /// if the file isn't cached we will request the fetcher to fetch it
    /// @param file    file name
    pub async fn request(
        &self,
        file: &String,
    ) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        // where we expect the file in storage
        let path = self.blob_location(file).await?;

        // this tokio::select! abonination is needed
        // to get scoped fetch_jobs so the lock gets released again...
        let active_job = tokio::select! {
            mut fetch_jobs = self.fetch_jobs.lock() => {
                match fetch_jobs.get(file) {
                    // fetch job running so we should wait
                    Some(job) => Some(job.clone()),
                    // no running fetch job
                    None => {
                        if path.is_file() {
                            // file should always fully exist in this case
                            println!("Cache hit on {}", file);
                            return Ok(path.to_path_buf());
                        } else {
                            // not fetched yet, this thread should fetch
                            fetch_jobs.insert(file.to_string(), Arc::new(Notify::new()));
                            None
                        }
                    }
                }
            }
        };

        // wait outside the above to get lock on fetch_jobs released
        if let Some(active_job) = active_job {
            println!("Already fetching {} - waiting until complete", file);
            active_job.notified().await;
            if path.is_file() {
                // usually the file should exist now
                // unless an error ocurred
                return Ok(path.to_path_buf());
            }
        }

        // then ask fetcher
        if self.fetcher.fetch(file, self).await.is_err() {
            // cleanup failed file
            if path.is_file() {
                fs::remove_file(&path)
                    .await
                    .expect("could not clean up bad fetch");
            }
            // if we have tasks waiting notify one to retry, else drop the job
            tokio::select! {
                mut fetch_jobs = self.fetch_jobs.lock() => {
                    if let Some(notify) = fetch_jobs.get(file) {
                        if Arc::strong_count(notify) > 1 {
                            eprintln!("Notifying waiting threads to retry download for {}", file);
                            notify.notify_one();
                            return Err(format!("Could not download file {}", file).into());
                        } else {
                            eprintln!("No waiting threads - not retrying download for {}", file);
                            fetch_jobs.remove(file);
                        }
                    }
                }
            }
        }

        // if we successfully fetched, remove job and notify all
        if let Some(notify) = self.fetch_jobs.lock().await.remove(file) {
            println!("Finished downloading {}", file);
            notify.notify_waiters();
        }

        // finish this thread
        if path.is_file() {
            return Ok(path.to_path_buf());
        }

        Err(format!("Could not download file {}", file).into())
    }
}
