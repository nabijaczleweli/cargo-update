use std::process::Command;
use std::path::Path;
use std::env;

fn main() {
    if cfg!(not(target_os = "windows")) {
        return;
    }

    let out_dir = env::var("OUT_DIR").unwrap();

    if cfg!(target_env = "gnu") {
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

    if cfg!(target_env = "msvc") {
        // We'll create a `.res` file here, but name it as `.lib`,
        // so that it can be found and linked correctly.
        // Yes, `.res` may be linked directly, like `.lib` and `.o`.
        // See `#[link(name="cargo-install-update-manifest")]` in main.rs.
        let out_res = format!("{}\\cargo-install-update-manifest.lib", out_dir);

        Command::new("rc")
            .args(&["/fo", &out_res, "cargo-install-update-manifest.rc"])
            .status()
            .unwrap();
    }

    println!("cargo:rustc-link-search=native={}", out_dir);
}
