use reqwest;

use crate::utils;
use crate::blob_storage::BlobStorage;

enum Layout {
    FileNameHashBlake2B,
}

/// get the mirror layout
/// for now this just matches that of the master mirror
/// TODO: actually make this a proper lookup
/// TODO: cache the layout.conf files
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

/// attempt to fetch a distfile from
/// a gentoo mirror
/// @param url   Base url of the mirror
/// @param file  Name of the distfile
pub async fn fetch_mirror(url: String, file: String, store: BlobStorage) -> Result<(), Box<dyn std::error::Error>> {
    let layout = mirror_layout(url.clone()).await;
    let full_url = match layout {
        Ok(Layout::FileNameHashBlake2B) => format!("{}/distfiles/{}/{}",
            url.clone(), utils::filename_hash_dir_blake2b(file.clone()).unwrap(), file.clone()),
        Err(_) => return Err(
            format!("Don't know how to handle layout.conf for mirror {}: {}",
                url.clone(), layout.err().unwrap().to_string()).into()
            ),
    };
    // we don't need layout but rustc complains
    // that it "maybe used later"
    layout.unwrap();

    println!("Fetching {}", full_url.clone());
    let response = reqwest::get(full_url).await?;
    response.error_for_status_ref()?;

    let mut stream = response.bytes_stream();
    store.store(file.clone(), &mut stream).await?;

    Ok(())
}
