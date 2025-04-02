use std::env;
use std::fs::{File, read_to_string};
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_vars.rs");
    let mut f = File::create(&dest_path).unwrap();

    // python interpreter used for portage integration
    let portage_python = env::var("PORTAGE_PYTHON").unwrap_or("python3".into());

    // read SRC_URI helper scrip to variable
    let src_uri_helper_py = read_to_string("meta/src_uri_helper.py").unwrap();

    // Write the variables to the generated file.
    write!(
        f,
        "pub const PORTAGE_PYTHON: &str = \"{}\";\npub const SRC_URI_HELPER_PY: &str = \"{}\";\n",
        portage_python.escape_default(), src_uri_helper_py.escape_default()
    )
    .unwrap();
    
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=meta/src_uri_helper.py");
    println!("cargo:rerun-if-env-changed=PORTAGE_PYTHON");
}
