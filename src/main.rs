use clap::Parser;
use rocket::{Build, Rocket};
use std::sync::{Arc, Mutex};
use tokio::task;

// import vars from build.rs
include!(concat!(env!("OUT_DIR"), "/build_vars.rs"));

// modules in this crate
mod blob_storage;
mod config;
mod ebuild_parser;
mod fetcher;
mod frontend;
mod manifest_walker;
mod repo_db;
mod repo_syncer;
mod utils;

use crate::blob_storage::BlobStorage;
use crate::config::Config;
use crate::repo_db::RepoDB;
use crate::repo_syncer::RepoSyncer;

/// Portage Distfile Cacher
#[derive(Parser, Debug)]
struct Args {
    /// Config File (defaults to ${PWD}/portcache.toml)
    #[arg(short, long)]
    config: Option<String>,
}

struct SharedData {
    /// BlobStorage for requesting blobs
    blob_storage: BlobStorage,
}

/// Main
#[rocket::launch]
async fn rocket() -> Rocket<Build> {
    let args = Args::parse();

    let config = Config::parse(args.config).unwrap_or_else(|e| {
        eprintln!("Failed to parse config: {}", e);
        std::process::exit(1);
    });

    let repo_db = Arc::new(Mutex::new(match RepoDB::new(&config) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    }));

    let repo_sync = RepoSyncer::new(&config, repo_db.clone()).await.unwrap();
    task::spawn(repo_sync.start());

    let storage = BlobStorage::new(&config)
        .await
        .expect("Failed to initialize blob storage");

    let cfg = rocket::config::Config {
        address: config.server.address,
        port: config.server.port,
        ..rocket::config::Config::default()
    };

    let shared = SharedData {
        blob_storage: storage,
    };

    rocket::custom(cfg).manage(shared).mount(
        "/",
        rocket::routes![frontend::layout_conf, frontend::distfiles],
    )
}
