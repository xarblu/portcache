use std::sync::Arc;

use rocket::http;
use rocket::response::stream::ReaderStream;
use rocket::routes;
use rocket::tokio::fs::File;
use rocket::{State, get};

use crate::blob_storage::BlobStorage;
use crate::config::Config;
use crate::utils;

struct SharedData {
    /// BlobStorage for requesting blobs
    storage: BlobStorage,
}

/// the layout.conf file indicating how files
/// are structures in this mirror
/// for now we just use the filename-hash mode
/// using the first 2 chars of the hex encoded blake2b521
#[get("/distfiles/layout.conf")]
async fn layout_conf() -> &'static str {
    "[structure]\n0=filename-hash BLAKE2B 8\n"
}

/// map requests to distfiles
#[get("/distfiles/<digest>/<file>")]
async fn distfiles(
    digest: &str,
    file: &str,
    shared: &State<SharedData>,
) -> Result<ReaderStream![File], http::Status> {
    // verify that digest matches file
    match utils::filename_hash_dir_blake2b(&file.to_string()) {
        Ok(x) if x == *digest => {}
        Ok(x) => {
            eprintln!(
                "Bad digest for file {}: Expected {}, Got {}",
                file, x, digest
            );
            return Err(http::Status::BadRequest);
        }
        Err(_) => {
            eprintln!("Something went wrong when calculating digest for {}", file);
            return Err(http::Status::InternalServerError);
        }
    }

    // file can be anything but not contain a / path seperator
    // rocket already shouldn't match files with / since that's implies a different route
    if digest.contains("/") {
        eprintln!("Received file with bad name: {}", file);
        return Err(http::Status::BadRequest);
    }

    let blob = match shared.storage.request(&file.to_string()).await {
        Ok(b) => b,
        Err(_) => return Err(http::Status::NotFound),
    };
    let file = File::open(blob).await.map_err(|e| e.to_string()).unwrap();
    Ok(ReaderStream::one(file))
}

/// launch the frontend webserver
pub async fn launch(config: Arc<Config>) -> Result<(), String> {
    let cfg = rocket::config::Config {
        address: config.server.address,
        port: config.server.port,
        ..rocket::config::Config::default()
    };

    let storage = BlobStorage::new(&config)
        .await
        .expect("Failed to initialize blob storage");
    let _ = rocket::custom(cfg)
        .manage(SharedData { storage })
        .mount("/", routes![layout_conf, distfiles])
        .launch()
        .await;

    Ok(())
}
