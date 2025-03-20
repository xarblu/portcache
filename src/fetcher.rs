use reqwest;
use std::path::PathBuf;

use crate::utils;
use crate::blob_storage::BlobStorage;
use crate::config;
use crate::manifest_walker::ManifestWalker;
use crate::ebuild_parser::Ebuild;

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

    /// root of the repo storage
    repo_root: PathBuf,
}

impl Fetcher {
    /// create a new Fetcher
    pub async fn new(config: &config::Config) -> Result<Self, String> {
        let mut mirrors: Vec<Mirror> = Vec::new();
        let repo_root = config.repo.storage_root.clone();

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

        Ok(Self { mirrors, next_mirror: 0, repo_root })
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

    /// utility method for fetching from a Gentoo mirror
    ///
    /// @param file  Name of the distfile
    /// @param store BlobStorage use for storing the file
    async fn fetch_mirror(&mut self, file: &String, store: &BlobStorage) -> Result<(), Box<dyn std::error::Error>> {
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

    /// utility method for fetching from SRC_URI
    ///
    /// @param file  Name of the distfile
    /// @param store BlobStorage use for storing the file
    async fn fetch_src_uri(&mut self, file: &String, store: &BlobStorage) -> Result<(), Box<dyn std::error::Error>> {

        let repos = self.repo_root.read_dir()?
            .filter_map(|x| match x {
                Ok(x) if x.path().is_dir() => Some(x),
                Ok(_) => None,
                Err(_) => None
            });
        
        // look through manifests and find a matching one
        let mut src_manifest = None;
        for repo in repos {
            println!("Checking repo {}", repo.path().to_string_lossy());
            let manifests = ManifestWalker::new(repo.path()).map_err(|e| e.to_string())?;
            src_manifest = match manifests.search(file.clone()).await {
                Ok(manifest) => manifest,
                Err(_) => continue,
            };

            if src_manifest.is_some() {
                break;
            }
        }

        // can't find file via SRC_URI either...
        if src_manifest.is_none() {
            return Err(format!("Could not find {} in Manifest of any configured repo", &file).into());
        }

        // we found a match - yay
        // let's parse all related ebuilds
        let ebuilds = src_manifest.unwrap().parent().unwrap()
            .read_dir().map_err(|e| e.to_string())?
            .filter_map(|x| match x {
                Ok(x) => match x.path().extension() {
                    Some(y) if y == "ebuild" => Some(x),
                    Some(_) => None,
                    None => None
                },
                Err(_) => None
            });
        
        let mut src_uri = None;
        for ebuild in ebuilds {
            println!("Checking {}", ebuild.path().to_string_lossy());
            let parsed = Ebuild::parse(ebuild.path()).await.map_err(|e| e.to_string())?;

            if let Some(url) = parsed.src_uri().get(file) {
                src_uri = Some(url.to_owned());
            }
        }
        
        if src_uri.is_none() {
            return Err(format!("Could not find {} in any ebuild belonging to Manifest", &file).into());
        }

        // try all urls
        for url in src_uri.unwrap() {
            println!("Fetching {}", url.clone());

            let mut stream = match reqwest::get(url).await {
                Err(e) => { eprintln!("{}", e); continue; },
                Ok(response) => match response.error_for_status_ref() {
                    Err(e) => { eprintln!("{}", e); continue; },
                    Ok(_) => response.bytes_stream()
                }
            };

            store.store(file.clone(), &mut stream).await?;
        }

        Ok(())
    }

    /// attempt to fetch a distfile
    ///  1. try from a gentoo mirror
    ///  2. try parsing from SRC_URI
    /// @param file  Name of the distfile
    /// @param store BlobStorage use for storing the file
    pub async fn fetch(&mut self, file: String, store: BlobStorage) -> Result<(), Box<dyn std::error::Error>> {
        // first try a mirror fetch
        match self.fetch_mirror(&file, &store).await {
            Ok(_) => return Ok(()),
            Err(e) => eprintln!("Mirror fetch failed: {}", e.to_string())
        }

        // then try a SRC_URI fetch
        match self.fetch_src_uri(&file, &store).await {
            Ok(_) => return Ok(()),
            Err(e) => eprintln!("SRC_URI fetch failed: {}", e.to_string())
        }

        Err(format!("All fetches failed for {}", &file).into())
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
