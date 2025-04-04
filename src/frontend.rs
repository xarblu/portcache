use std::sync::Arc;

use futures::lock::Mutex;
use rocket::http;
use rocket::response::stream::ReaderStream;
use rocket::routes;
use rocket::tokio::fs::File;
use rocket::{State, get};

use crate::blob_storage::BlobStorage;
use crate::config::Config;

struct SharedData {
    /// BlobStorage wrapped in a Mutex
    /// for inner mutability required by rocket
    storage: Mutex<BlobStorage>,
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
    // sanity checks
    // digest should be hex decodable and exactly 2 bytes
    if digest.len() != 2 {
        eprintln!("Received digest with bad length: {}", digest);
        return Err(http::Status::BadRequest);
        //return (http::Status::BadRequest, None);
    } else if hex::decode(digest).is_err() {
        eprintln!("Received digest with bad content: {}", digest);
        return Err(http::Status::BadRequest);
    }

    // file can be anything but not contain a / path seperator
    // rocket already shouldn't match files with / since that's implies a different route
    if digest.contains("/") {
        eprintln!("Received file with bad name: {}", file);
        return Err(http::Status::BadRequest);
    }

    let mut storage = shared.storage.lock().await;
    let blob = match storage.request(digest.to_string(), file.to_string()).await {
        Ok(b) => b,
        Err(_) => return Err(http::Status::NotFound),
    };
    let file = File::open(blob).await.map_err(|e| e.to_string()).unwrap();
    Ok(ReaderStream::one(file))
}

/// launch the frontend webserver
pub async fn launch(config: Arc<Config>) -> Result<(), String> {
    let cfg = rocket::config::Config {
        address: config.server.address.clone(),
        port: config.server.port.clone(),
        ..rocket::config::Config::default()
    };

    let storage = BlobStorage::new(&config)
        .await
        .expect("Failed to initialize blob storage");
    let _ = rocket::custom(cfg)
        .manage(SharedData {
            storage: Mutex::new(storage),
        })
        .mount("/", routes![layout_conf, distfiles])
        .launch()
        .await;

    Ok(())
}
