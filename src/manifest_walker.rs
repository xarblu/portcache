use std::ffi::OsStr;
use std::path::PathBuf;
use tokio::fs;
use tokio::io;
use tokio::io::AsyncBufReadExt;
use walkdir::WalkDir;

pub struct ManifestEntry {
    /// origin Manifest file
    origin: PathBuf,

    /// file name
    file: String,

    /// file size in bytes
    size: u32,

    /// blake2b checksum
    blake2b: Option<String>,

    /// sha512 checksum
    sha512: Option<String>,
}

impl ManifestEntry {
    /// parse a manifest entry from a manifest line
    pub fn parse(origin: PathBuf, line: &String) -> Result<ManifestEntry, String> {
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
            origin,
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

    /// search for input string in Manifest files
    /// returns on first match
    ///
    /// @param pattern  The pattern to search
    /// @return         Ok(PathBuf) to the matched Manifest when found
    ///                 Ok(None) if not
    ///                 Err on IO error
    pub async fn search(&self, pattern: String) -> Result<Option<PathBuf>, String> {
        // Manifests are always exactly at the 2nd level (category/package/Manifest)
        let manifests = WalkDir::new(self.root.as_os_str())
            .min_depth(3)
            .max_depth(3)
            .into_iter()
            .filter_entry(|e| e.file_name() == "Manifest");

        for manifest in manifests {
            // skip bad files
            let path = match manifest {
                Ok(manifest) => manifest.path().to_owned(),
                Err(_) => continue,
            };

            let mut lines = match fs::File::open(path.clone()).await {
                Ok(file) => io::BufReader::new(file).lines(),
                Err(_) => continue,
            };

            'parse_lines: while let Some(line) = match lines.next_line().await {
                Ok(line) => line,
                Err(_) => break 'parse_lines,
            } {
                if line.contains(&pattern) {
                    println!("Found {} in {}", &pattern, path.to_string_lossy());
                    return Ok(Some(PathBuf::from(&path)));
                }
            }
        }

        Ok(None)
    }
}
