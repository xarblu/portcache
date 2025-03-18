use reqwest;

use crate::utils;
use crate::blob_storage::BlobStorage;
use crate::config;

#[derive(Clone)]
enum Layout {
    FileNameHashBlake2B,
}

#[derive(Clone)]
struct Mirror {
    /// sanitized url of the mirror
    url: String,

    /// layout of the mirror
    layout: Layout,
}

#[derive(Clone)]
pub struct Fetcher {
    /// list of mirrors to fetch from
    mirrors: Vec<Mirror>,

    /// next mirror tracker for round robin load balancing
    next_mirror: usize,
}

impl Fetcher {
    /// create a new Fetcher
    pub async fn new(config: &config::Config) -> Result<Self, String> {
        let mut mirrors: Vec<Mirror> = Vec::new();

        for url in config.fetcher.mirrors.clone() {
            // sanitize url
            let url = String::from(url.trim_end_matches("/"));

            // get mirror layout, if that fails don't add the mirror
            let layout = mirror_layout(url.clone()).await.map_err(|e| e.to_string());
            if layout.is_err() {
                eprint!("Failed to get valid layout.conf from mirror {}. Ignoring mirror.", url);
                continue;
            }

            mirrors.push(Mirror { url, layout: layout.unwrap() })
        }

        if mirrors.is_empty() {
            return Err(format!("Mirror list is empty"));
        }

        Ok(Self { mirrors, next_mirror: 0 })
    }

    /// select a mirror in round robin fashion
    fn select_mirror(&mut self) -> &Mirror {
        let mirror = &self.mirrors[self.next_mirror];

        self.next_mirror = if self.next_mirror == self.mirrors.len() - 1 {
            0
        } else {
            self.next_mirror + 1
        };

        mirror
    }

    /// attempt to fetch a distfile from
    /// a gentoo mirror
    /// @param file  Name of the distfile
    /// @param store BlobStorage use for storing the file
    pub async fn fetch(&mut self, file: String, store: BlobStorage) -> Result<(), Box<dyn std::error::Error>> {
        let mirror = self.select_mirror();
        let full_url = match mirror.layout {
            Layout::FileNameHashBlake2B => format!("{}/distfiles/{}/{}",
                mirror.url.clone(), utils::filename_hash_dir_blake2b(file.clone()).unwrap(), file.clone()),
        };

        println!("Fetching {}", full_url.clone());
        let response = reqwest::get(full_url).await?;
        response.error_for_status_ref()?;

        let mut stream = response.bytes_stream();
        store.store(file.clone(), &mut stream).await?;

        Ok(())
    }
}

/// get the mirror layout
/// for now this just matches that of the master mirror
/// TODO: actually make this a proper lookup
async fn mirror_layout(url: String) -> Result<Layout, Box<dyn std::error::Error>> {
    let layout = reqwest::get(format!("{}/{}", url, "distfiles/layout.conf"))
        .await?
        .text()
        .await?;

    match layout.as_str() {
        "[structure]\n0=filename-hash BLAKE2B 8\n" => Ok(Layout::FileNameHashBlake2B),
        _ => Err(format!("Unknown layout in layout.conf: {}", layout).into()),
    }
}
