use std::sync::Arc;

use clap::Parser;
use repo_syncer::RepoSyncer;
use tokio::signal::unix::SignalKind;
use tokio_util::sync::CancellationToken;

// import vars from build.rs
include!(concat!(env!("OUT_DIR"), "/build_vars.rs"));

// modules in this crate
mod blob_storage;
mod config;
mod ebuild_parser;
mod fetcher;
mod frontend;
mod manifest_walker;
mod repo_syncer;
mod utils;

/// Portage Distfile Cacher
#[derive(Parser, Debug)]
struct Args {
    /// Config File (defaults to ${PWD}/portcache.toml)
    #[arg(short, long)]
    config: Option<String>,
}

/// Main
#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config = Arc::new(config::Config::parse(args.config).unwrap_or_else(|e| {
        eprintln!("Failed to parse config: {}", e);
        std::process::exit(1);
    }));

    let mut tasks = tokio::task::JoinSet::new();
    let token = CancellationToken::new();

    tasks.spawn(frontend::launch(config.clone()));

    let repo_sync = RepoSyncer::new(config.clone()).await.unwrap();
    tasks.spawn(repo_sync.start(token.clone()));

    // signal handler so we can shut things down
    tasks.spawn(async move {
        let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt()).unwrap();
        let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate()).unwrap();

        tokio::select! {
            _ = sigint.recv() => {},
            _ = sigterm.recv() => {},
        }

        token.cancel();
        Ok(())
    });

    tasks.join_all().await;
}
