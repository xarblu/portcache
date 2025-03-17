use blake2::{Blake2b512, Digest};

/// convert a distfile name to the directory it's
/// supposed to be in i.e. the first 2 bytes of the 8 byte BLAKE2B
/// https://github.com/gentoo/portage/blob/portage-3.0.67/lib/portage/checksum.py#L27
/// @param name  File name to hash
pub fn filename_hash_dir_blake2b(name: String) -> Result<String, Box<dyn std::error::Error>> {
    let mut hasher = Blake2b512::new();
    hasher.update(name.as_bytes());
    let res = hasher.finalize();
    Ok(hex::encode(&res[..1]))
}
