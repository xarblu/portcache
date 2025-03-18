use clap::Parser;

mod frontend;
mod utils;
mod config;
mod fetcher;
mod blob_storage;

use crate::blob_storage::BlobStorage;

/// Portage Distfile Cacher
#[derive(Parser, Debug)]
struct Args {
    /// Config File (defaults to ${PWD}/portcache.toml)
    #[arg(short,long)]
    config: Option<String>,
}

/// Main
#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config = config::Config::parse(args.config)
        .unwrap_or_else(|e| {
            eprintln!("Failed to parse config: {}", e.to_string());
            std::process::exit(1);
        });

    let storage = BlobStorage::new(config.storage.clone())
        .await
        .unwrap_or_else(|e| {
            eprintln!("Could not initialize blob storage: {}", e.to_string());
            std::process::exit(1);
        });

    let _rocket = frontend::launch(config.clone(), storage.clone())
        .await
        .unwrap_or_else(|e| {
            eprintln!("Failed to launch rocket: {}", e.to_string());
            std::process::exit(1);
        });
}
