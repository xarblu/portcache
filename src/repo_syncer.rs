use git2::Direction;
use git2::Repository;
use git2::ResetType;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::time;

use crate::config;

/// struct to clone and sync portage repos
pub struct RepoSyncer {
    /// interval in which to sync repos
    sync_interval: time::Duration,

    /// path where the repos o sync are stored
    storage_root: PathBuf,
}

impl RepoSyncer {
    /// initialize the reposyncer
    /// will create repo_storage_root if it doesn't exist
    /// and clone all configured repos to it
    ///
    /// @param config  a reference to Config
    /// @returns Err   when repo_storage_root couldn't be created or isn't writable
    pub async fn new(config: Arc<config::Config>) -> Result<Self, String> {
        let sync_interval = time::Duration::from_secs(config.repo.sync_interval * 60);
        let storage_root = config.storage.location.join("repos");
        let repos = config.repo.repos.clone();

        if !storage_root.is_dir() {
            fs::create_dir(storage_root.as_path())
                .await
                .map_err(|e| format!("Failed to create repo storage root: {}", e))?;
        }

        for repo in repos {
            let mut path = storage_root.clone();
            match repo.split("/").last() {
                Some(name) => path.push(name),
                None => {
                    eprintln!("Could't get name for repo: {}", repo);
                    continue;
                }
            };

            if path.is_dir() {
                println!(
                    "Skipping setup of existing repo at {}",
                    path.to_string_lossy()
                );
                continue;
            }

            // perform a shallow clone since we really don't need old commits here
            let mut options = git2::FetchOptions::new();
            options.depth(1);

            let mut builder = git2::build::RepoBuilder::new();
            builder.fetch_options(options);
            match builder.clone(repo.as_str(), path.as_path()) {
                Ok(_) => println!(
                    "Successfully cloned repo {} to {}",
                    repo,
                    path.to_string_lossy()
                ),
                Err(e) => eprintln!(
                    "Failed cloning repo {} to {}: {}",
                    repo,
                    path.to_string_lossy(),
                    e
                ),
            }
        }

        Ok(Self {
            sync_interval,
            storage_root,
        })
    }

    /// start RepoSyncer
    /// this is expected to be called from a tokio::spawn
    /// and consumes RepoSyncer
    pub async fn start(self) -> Result<(), String> {
        let mut interval = time::interval(self.sync_interval);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.sync().await {
                        eprintln!("Sync failed: {}", e);
                    }
                }
            }
        }
    }

    /// perform a sync for all repos in storage_root
    ///
    /// @returns
    async fn sync(&self) -> Result<(), String> {
        let repos = self
            .storage_root
            .clone()
            .read_dir()
            .map_err(|e| e.to_string())?;

        let mut failed = Vec::new();
        for entry in repos {
            if let Err(e) = entry {
                eprintln!(
                    "IO error while reading {}: {}",
                    self.storage_root.clone().to_string_lossy(),
                    e
                );
                continue;
            }

            let path = entry.unwrap().path();
            println!("Syncing repo: {}", path.to_string_lossy());
            let repo = match Repository::open(&path) {
                Ok(repo) => repo,
                Err(e) => {
                    eprintln!("Failed to open repo: {}", e);
                    failed.push(String::from(path.to_string_lossy()));
                    continue;
                }
            };

            let mut remote = match repo.find_remote("origin") {
                Ok(remote) => remote,
                Err(_) => {
                    eprintln!(
                        "Repository at {} doesn't have remote \"origin\" to fetch from - skipping",
                        path.to_string_lossy()
                    );
                    failed.push(String::from(path.to_string_lossy()));
                    continue;
                }
            };

            if let Err(e) = remote.connect(Direction::Fetch) {
                eprintln!("Failed to connect to remote: {}", e);
                failed.push(String::from(path.to_string_lossy()));
                continue;
            }

            let default_branch = match remote.default_branch() {
                Ok(branch) => branch.as_str().unwrap_or("refs/heads/main").to_string(),
                Err(e) => {
                    eprintln!("Failed to get default branch: {}", e);
                    failed.push(String::from(path.to_string_lossy()));
                    continue;
                }
            };

            // perform a shallow clone since we really don't need old commits here
            let mut options = git2::FetchOptions::new();
            options.depth(1);

            if let Err(e) =
                remote.fetch(&[default_branch.clone().as_str()], Some(&mut options), None)
            {
                eprintln!("Failed to fetch repo: {}", e);
                failed.push(String::from(path.to_string_lossy()));
                continue;
            }

            let remote_tracking = format!(
                "refs/remotes/origin/{}",
                default_branch
                    .clone()
                    .split("/")
                    .last()
                    .unwrap_or("main")
                    .to_owned()
            );
            let fetch_head = match repo.find_reference(remote_tracking.as_str()) {
                Ok(head) => head,
                Err(e) => {
                    eprintln!("Failed to find fetch_head in repo: {}", e);
                    failed.push(String::from(path.to_string_lossy()));
                    continue;
                }
            };

            let target_commit = match fetch_head.peel_to_commit() {
                Ok(commit) => commit,
                Err(e) => {
                    eprintln!("Failed to find commit for fetch_head in repo: {}", e);
                    failed.push(String::from(path.to_string_lossy()));
                    continue;
                }
            };

            if let Err(e) = repo.reset(target_commit.as_object(), ResetType::Hard, None) {
                eprintln!("Failed to reset repo to target commit: {}", e);
                failed.push(String::from(path.to_string_lossy()));
                continue;
            }
        }

        if failed.is_empty() {
            Ok(())
        } else {
            Err(format!("Failed repos: {}", failed.join(", ")))
        }
    }
}
