use std::ffi::OsStr;
use std::path::PathBuf;
use tokio::fs;
use tokio::io;
use tokio::io::AsyncBufReadExt;
use walkdir::WalkDir;

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
