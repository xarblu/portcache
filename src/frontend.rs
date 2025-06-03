use rocket::http;
use rocket::response::stream::ReaderStream;
use rocket::tokio::fs::File;
use rocket::{State, get};

use crate::SharedData;
use crate::utils;

/// the layout.conf file indicating how files
/// are structures in this mirror
/// for now we just use the filename-hash mode
/// using the first 2 chars of the hex encoded blake2b521
#[get("/distfiles/layout.conf")]
pub(crate) async fn layout_conf() -> &'static str {
    "[structure]\n0=filename-hash BLAKE2B 8\n"
}

/// map requests to distfiles
#[get("/distfiles/<digest>/<file>")]
pub(crate) async fn distfiles(
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

    let blob = match shared.blob_storage.request(&file.to_string()).await {
        Ok(b) => b,
        Err(_) => return Err(http::Status::NotFound),
    };
    let file = File::open(blob).await.map_err(|e| e.to_string()).unwrap();
    Ok(ReaderStream::one(file))
}
