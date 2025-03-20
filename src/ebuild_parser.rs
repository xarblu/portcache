use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::io::AsyncBufReadExt;
use tokio::io;
use tokio::process::Command;
use std::process::Stdio;

/// parse an ebuild file
pub struct Ebuild {
    /// path to ebuild
    path: PathBuf,

    /// HasMap with filename as keys and urls as values
    src_uri: HashMap<String, String>
}

impl Ebuild {
    /// parse an ebuild file
    /// TODO: do more of this internally
    /// TODO: better cleanup and env setup
    ///
    /// @param path  PathBuf to ebuild
    pub async fn parse(path: PathBuf) -> Result<Self, String> {
        let path_str = match path.as_os_str().to_str() {
            Some(s) => s,
            None => return Err(format!("Could not convert path to str"))
        };

        // full CATEGORY/PN-PV
        let full_pkg = format!("{}/{}",
            path.clone().parent().unwrap().file_name().unwrap().to_string_lossy(),
            path.clone().file_name().unwrap().to_string_lossy(),
        );

        // let plain ebuild handle the setup
        let ebuild = Command::new("ebuild")
            .args(&[path_str, "clean", "setup"])
            .status()
            .await
            .expect("ebuild failed to run");

        if !ebuild.success() {
            return Err(format!("Failed to run ebuild setup for {}", path.clone().to_string_lossy()));
        }

        // then let bash + perl handle the preprocessing
        let mut preprocessor = Command::new("bash")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("bash failed to run");

        let mut stdin = preprocessor.stdin.take().expect("failed to opend stdin");
        let script = [
            format!("source /var/tmp/portage/{}/temp/environment >/dev/null || exit 1", full_pkg),
            String::from("export SRC_URI >/dev/null || exit 1"),
            String::from("perl -e '$src_uri = $ENV{\"SRC_URI\"}; while ($src_uri =~ /\\s*(?:\\S+\\?\\s+\\(\\s+)*\\s*(\\S+)(?:\\s+->\\s+(\\S+))?(?:\\s+\\))*\\s*/g) { print \"$1\"; print \" -> $2\" if defined $2; print \"\n\"; }' || exit 1"),
        ].join("\n");
        tokio::spawn(async move {
            stdin.write_all(script.as_bytes()).await.expect("failed to write stdin");
        });

        let stdout = preprocessor.stdout.take().expect("failed to open stdout");
        let mut src_uri_lines = io::BufReader::new(stdout).lines();
        
        tokio::spawn(async move {
            let _ = preprocessor.wait().await
                .expect("preprocessor encountered an error");
        });

        let mut src_uri = HashMap::new();
        while let Some(line) = src_uri_lines.next_line().await.map_err(|e| e.to_string())? {
            let spec: Vec<&str> = line.split(" -> ").collect();
            
            // expect format /url( -> name)?/
            if spec.len() < 1 || spec.len() > 2 {
                return Err(format!("ebuild preprocessor returned garbage data: {}", line));
            }
            
            // url is always the 1st part
            let url = String::from(spec[0]);

            // name is either 2nd part or last url component
            let name = if spec.len() == 2 {
                String::from(spec[1])
            } else {
                String::from(url.split("/").last().unwrap())
            };

            src_uri.insert(name, url);
        }

        Ok(Self { path, src_uri })
    }

    /// get reference to src_uri HashMap
    pub fn src_uri(&self) -> &HashMap<String, String> {
        &self.src_uri
    }
}
