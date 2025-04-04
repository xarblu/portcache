use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::PORTAGE_PYTHON;
use crate::SRC_URI_HELPER_PY;

/// structure returned by portage helper
type SrcUriObj = HashMap<String, Vec<String>>;

/// parse an ebuild file
pub struct Ebuild {
    /// object containing the SRC_URIs
    src_uri: SrcUriObj,
}

impl Ebuild {
    /// parse an ebuild file
    ///
    /// @param path  PathBuf to ebuild
    pub async fn parse(path: PathBuf) -> Result<Self, String> {
        let ebuild = match path.as_os_str().to_str() {
            Some(s) => s,
            None => return Err(format!("Could not convert path to str")),
        };

        // hook into portage python API for processing ebuilds
        let mut preprocessor = Command::new(PORTAGE_PYTHON)
            .args(["-", ebuild])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("ebuild processor failed to run");

        // hand script to python
        let mut stdin = preprocessor.stdin.take().expect("failed to opend stdin");
        tokio::spawn(async move {
            stdin
                .write_all(SRC_URI_HELPER_PY.as_bytes())
                .await
                .expect("failed to write stdin");
        });

        let stdout = preprocessor.stdout.take().expect("failed to open stdout");
        let mut src_uri_json = String::new();
        io::BufReader::new(stdout)
            .read_to_string(&mut src_uri_json)
            .await
            .map_err(|e| e.to_string())?;

        tokio::spawn(async move {
            let _ = preprocessor
                .wait()
                .await
                .expect("preprocessor encountered an error");
        });

        let src_uri: SrcUriObj = serde_json::from_str(&src_uri_json).map_err(|e| e.to_string())?;

        Ok(Self { src_uri })
    }

    /// get reference to src_uri
    pub fn src_uri(&self) -> &SrcUriObj {
        &self.src_uri
    }
}
