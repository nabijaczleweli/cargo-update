use std::process::Command;
use std::path::Path;
use std::env;

fn main() {
    if cfg!(target_os = "windows") && cfg!(target_env = "gnu") {
        let out_dir = env::var("OUT_DIR").unwrap();

        Command::new("windres")
            .args(&["--input", "cargo-install-update-manifest.rc", "--output-format=coff", "--output"])
            .arg(&format!("{}/cargo-install-update-manifest.res", out_dir))
            .status()
            .unwrap();

        Command::new("ar")
            .args(&["crs", "libcargo-install-update-manifest.a", "cargo-install-update-manifest.res"])
            .current_dir(&Path::new(&out_dir))
            .status()
            .unwrap();

        println!("cargo:rustc-link-search=native={}", out_dir);
    }
}
