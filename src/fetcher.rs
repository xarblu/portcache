use futures::lock::Mutex;
use futures::stream::StreamExt;
use futures_core::stream::Stream;
use std::sync::Arc;
use tokio::{
    fs,
    io::{self, AsyncWriteExt},
};

use crate::blob_storage::BlobStorage;
use crate::config;
use crate::repo_db::RepoDB;
use crate::utils;

#[derive(Clone)]
enum Layout {
    FileNameHashBlake2B,
}

#[derive(Clone)]
struct Mirror {
    /// sanitized url of the mirror
    url: String,
}

pub struct Fetcher {
    /// list of mirrors to fetch from
    mirrors: Vec<Mirror>,

    /// next mirror tracker for round robin load balancing
    next_mirror: Mutex<usize>,

    /// repo database
    repo_db: Arc<RepoDB>,
}

impl Fetcher {
    /// create a new Fetcher
    pub async fn new(config: &config::Config, repo_db: Arc<RepoDB>) -> Result<Self, String> {
        let mut mirrors: Vec<Mirror> = Vec::new();

        for url in config.fetcher.mirrors.clone() {
            // sanitize url
            let url = String::from(url.trim_end_matches("/"));

            mirrors.push(Mirror { url })
        }

        if mirrors.is_empty() {
            return Err("Mirror list is empty".to_string());
        }

        Ok(Self {
            mirrors,
            next_mirror: Mutex::new(0),
            repo_db,
        })
    }

    /// select a mirror in round robin fashion
    async fn select_mirror(&self) -> &Mirror {
        let mut next = self.next_mirror.lock().await;
        let mirror = &self.mirrors[*next];
        *next = if *next == self.mirrors.len() - 1 {
            0
        } else {
            *next + 1
        };

        mirror
    }

    /// store a blob from a stream in the storage
    /// @param name  name of the blob
    /// @param blob  a bytes stream with the blob
    pub async fn store(
        &self,
        name: &String,
        blob_storage: &BlobStorage,
        blob: &mut (impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + std::marker::Unpin),
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = blob_storage.blob_location(name).await?;

        // file exists - for now just exit
        // although best case we don't even attempt to re-download
        if path.is_file() {
            return Ok(());
        }

        // create dir for this blob if needed
        // assert that parent is not / or empty
        assert!(path.parent().is_some());
        if !path.parent().unwrap().is_dir() {
            fs::create_dir(path.parent().unwrap()).await?;
        }

        // write file chunks
        let file = fs::File::create(&path).await?;
        let mut writer = io::BufWriter::new(file);

        while let Some(chunk) = blob.next().await {
            if chunk.is_err() {
                writer.flush().await?;
                eprintln!(
                    "Error while downloading {}: {}",
                    name.clone(),
                    chunk.err().unwrap()
                );
                fs::remove_file(&path).await?;
                return Err("Download failed".into());
            }

            writer.write_all(&chunk?).await?;
        }

        writer.flush().await?;

        Ok(())
    }

    /// utility method for fetching from Gentoo mirrors
    /// will try all configured mirrors before failing
    ///
    /// @param file  Name of the distfile
    /// @param store BlobStorage use for storing the file
    async fn fetch_mirror(&self, file: &String, store: &BlobStorage) -> Result<(), String> {
        for _ in 0..self.mirrors.len() {
            // select mirror
            let mirror = self.select_mirror().await;

            // get mirror layout and ignore mirror if it's invalid
            let layout = match mirror_layout(&mirror.url).await {
                Ok(layout) => layout,
                Err(e) => {
                    eprintln!(
                        "Ignoring mirror {} due to bad layout.conf: {}",
                        &mirror.url, e
                    );
                    continue;
                }
            };

            let full_url = match layout {
                Layout::FileNameHashBlake2B => format!(
                    "{}/distfiles/{}/{}",
                    mirror.url,
                    utils::filename_hash_dir_blake2b(file).unwrap(),
                    file
                ),
            };

            println!("Fetching {}", &full_url);

            let mut stream = match reqwest::get(&full_url).await {
                Err(e) => {
                    eprintln!("{}", e);
                    continue;
                }
                Ok(response) => match response.error_for_status_ref() {
                    Err(e) => {
                        eprintln!("{}", e);
                        continue;
                    }
                    Ok(_) => response.bytes_stream(),
                },
            };

            match self.store(file, store, &mut stream).await {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("GET {} failed: {}", &full_url, e);
                    continue;
                }
            };

            // only Ok when entire pipeline was success
            return Ok(());
        }

        Err(format!(
            "Couldn't fetch {} from any configured mirror",
            file
        ))
    }

    /// utility method for fetching from SRC_URI
    ///
    /// @param file  Name of the distfile
    /// @param store BlobStorage use for storing the file
    async fn fetch_src_uri(
        &self,
        file: &String,
        store: &BlobStorage,
    ) -> Result<(), Box<dyn std::error::Error>> {

        // try all uris
        let uris = match self.repo_db.get_src_uri(file).await {
            Ok(x) => x,
            Err(e) => return Err(e.into())
        };
        for uri in uris {
            println!("Fetching {}", &uri);

            let mut stream = match reqwest::get(uri).await {
                Err(e) => {
                    eprintln!("{}", e);
                    continue;
                }
                Ok(response) => match response.error_for_status_ref() {
                    Err(e) => {
                        eprintln!("{}", e);
                        continue;
                    }
                    Ok(_) => response.bytes_stream(),
                },
            };

            self.store(file, store, &mut stream).await?;
        }

        Ok(())
    }

    /// attempt to fetch a distfile
    ///  1. try from a gentoo mirror
    ///  2. try parsing from SRC_URI
    ///     @param file  Name of the distfile
    ///     @param store BlobStorage use for storing the file
    pub async fn fetch(&self, file: &String, store: &BlobStorage) -> Result<(), ()> {
        // first try a mirror fetch
        match self.fetch_mirror(file, store).await {
            Ok(_) => return Ok(()),
            Err(e) => eprintln!("Mirror fetch failed: {}", e),
        }

        // then try a SRC_URI fetch
        match self.fetch_src_uri(file, store).await {
            Ok(_) => return Ok(()),
            Err(e) => eprintln!("SRC_URI fetch failed: {}", e),
        }

        eprintln!("All fetches failed for {}", &file);
        Err(())
    }
}

/// get the mirror layout
/// for now this just matches that of the master mirror
/// TODO: actually make this a proper lookup
async fn mirror_layout(url: &String) -> Result<Layout, String> {
    let layout = match reqwest::get(format!("{}/{}", url, "distfiles/layout.conf")).await {
        Ok(res) => match res.text().await {
            Ok(text) => text,
            Err(e) => return Err(e.to_string()),
        },
        Err(e) => return Err(e.to_string()),
    };

    match layout.as_str() {
        "[structure]\n0=filename-hash BLAKE2B 8\n" => Ok(Layout::FileNameHashBlake2B),
        _ => Err(format!("Unknown layout in layout.conf: {}", layout)),
    }
}
