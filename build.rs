use std::process::Command;
use std::path::Path;
use std::env;

fn main() {
    if cfg!(not(target_os = "windows")) {
        return;
    }

    let out_dir = env::var("OUT_DIR").unwrap();

    if cfg!(target_env = "msvc") {
        // `.res`es are linkable under MSVC as well as normal libraries, for w/e reason.
        Command::new("rc")
            .args(&["/fo", &format!("{}/cargo-install-update-manifest.lib", out_dir), "cargo-install-update-manifest.rc"])
            .status()
            .unwrap();
    } else {
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
    }

    println!("cargo:rustc-link-search=native={}", out_dir);
}
