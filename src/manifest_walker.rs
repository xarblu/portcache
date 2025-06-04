use std::ffi::OsStr;
use std::path::PathBuf;
use tokio::fs;
use tokio::io;
use tokio::io::AsyncBufReadExt;
use walkdir::WalkDir;
use async_stream::stream;
use futures_core::stream::Stream;

pub struct ManifestEntry {
    /// origin Manifest file
    pub origin: PathBuf,

    /// file name
    pub file: String,

    /// file size in bytes
    pub size: u32,

    /// blake2b checksum
    pub blake2b: Option<String>,

    /// sha512 checksum
    pub sha512: Option<String>,
}

impl ManifestEntry {
    /// parse a manifest entry from a manifest line
    pub fn parse(origin: &PathBuf, line: &String) -> Result<ManifestEntry, String> {
        let mut file: Option<String> = None;
        let mut size: Option<u32> = None;
        let mut blake2b: Option<String> = None;
        let mut sha512: Option<String> = None;

        let mut parts = line.split_whitespace();
        while let Some(part) = parts.next() {
            match part {
                "DIST" => {
                    file = match parts.next() {
                        Some(s) => Some(s.to_string()),
                        None => {
                            return Err(format!(
                                "Expected file name after \"DIST\" in line \"{}\"",
                                &line
                            ));
                        }
                    };
                    size = match parts.next() {
                        Some(s) => match s.parse() {
                            Ok(i) => Some(i),
                            Err(e) => return Err(e.to_string()),
                        },
                        None => {
                            return Err(format!(
                                "Expected file size after \"DIST {}\" in line \"{}\"",
                                &file.unwrap(),
                                &line
                            ));
                        }
                    };
                }
                "BLAKE2B" => {
                    blake2b = match parts.next() {
                        Some(s) => Some(s.to_string()),
                        None => {
                            return Err(format!(
                                "Expected blake2b checksum after \"BLAKE2B\" in line \"{}\"",
                                &line
                            ));
                        }
                    };
                }
                "SHA512" => {
                    sha512 = match parts.next() {
                        Some(s) => Some(s.to_string()),
                        None => {
                            return Err(format!(
                                "Expected sha512 checksum after \"SHA512\" in line \"{}\"",
                                &line
                            ));
                        }
                    };
                }
                _ => (),
            }
        }

        // file name and size are required, checksums are optional for now
        if file.is_none() {
            return Err(format!("Line {} doesn't contain file name", &line));
        }

        if size.is_none() {
            return Err(format!("Line {} doesn't contain file size", &line));
        }

        Ok(ManifestEntry {
            origin: origin.to_owned(),
            file: file.unwrap(),
            size: size.unwrap(),
            blake2b,
            sha512,
        })
    }
}

/// walk through Manifest files in a ebuild tree
pub struct ManifestWalker {
    /// ebuild tree root
    root: PathBuf,
}

impl ManifestWalker {
    /// create a new ManifestWalker
    ///
    /// @param root  ebuild tree root
    /// @return      Self on success, Error when tree invalid
    pub fn new(root: PathBuf) -> Result<Self, String> {
        // in a valid tree we expect metadata/layout.conf to exist
        let layout_conf = PathBuf::from_iter([
            root.clone().as_os_str(),
            OsStr::new("metadata"),
            OsStr::new("layout.conf"),
        ]);

        if !layout_conf.is_file() {
            return Err("Could not find metadata/layout.conf in repo root \
                - this doesn't look like a valid repo"
                .to_string());
        }

        Ok(Self { root })
    }

    /// get a stream of all Manifest entries in the tree
    /// TODO: might be useful to return errors
    pub fn entries(&mut self) -> impl Stream<Item = ManifestEntry> {
        stream! {
            // initialise walkdir
            // Manifests are always exactly at the 2nd level (category/package/Manifest)
            let mut candidates = WalkDir::new(self.root.as_os_str())
                .min_depth(3)
                .max_depth(3)
                .into_iter();

            while let Some(file) = candidates.next() {
                let manifest = match file {
                    Ok(x) if x.file_name() == "Manifest" => PathBuf::from(x.path()),
                    Ok(_) => continue,
                    Err(_) => continue,
                };
                
                // create new line reader
                let mut lines = match fs::File::open(&manifest).await {
                    Ok(x) => io::BufReader::new(x).lines(),
                    Err(_) => continue,
                };

                // parse each line
                // on IO error the entire Manifest gets skipped
                // on parse error skip line
                loop {
                    let line = match lines.next_line().await {
                        Err(e) => {
                            // IO Error
                            eprintln!("IO error while parsing {}: {}", manifest.to_string_lossy(), e.to_string());
                            break;
                        },
                        Ok(maybe_eof) => match maybe_eof {
                            None => break, // EOF
                            Some(line) => line
                        }
                    };
                    
                    let ret = match ManifestEntry::parse(&manifest, &line) {
                        Ok(entry) => entry,
                        Err(e) => {
                            eprintln!("Parser error while parsing {}: {}", manifest.to_string_lossy(), e.to_string());
                            continue;
                        },
                    };

                    yield ret;
                }
            }
        }
    }
}
