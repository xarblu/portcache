use rocket::{get, State};
use rocket::response::stream::ReaderStream;
use rocket::routes;
use rocket::{Rocket,Ignite};
use rocket::tokio::fs::File;
use rocket::tokio::io;

use crate::blob_storage::BlobStorage;
use crate::config::Config;

/// the layout.conf file indicating how files
/// are structures in this mirror
/// for now we just use the filename-hash mode
#[get("/distfiles/layout.conf")]
async fn layout_conf() -> &'static str {
    "[structure]\n0=filename-hash BLAKE2B 8\n"
}

/// map requests to distfiles
/// TODO: sanitize variables
#[get("/distfiles/<digest>/<file>")]
async fn distfiles(digest: String, file: String, storage: &State<BlobStorage>) -> io::Result<ReaderStream![File]> {
    let path = storage.request(digest.clone(), file.clone()).await.expect("Failed to fetch");
    let file = File::open(path).await?;
    Ok(ReaderStream::one(file))
}

/// launch the frontend webserver
pub async fn launch(
    config: Config,
    storage: BlobStorage,
    ) -> Result<Rocket<Ignite>, rocket::Error> {
    rocket::build()
        .manage(config)
        .manage(storage)
        .mount("/", routes![layout_conf,distfiles])
        .launch()
        .await
}

