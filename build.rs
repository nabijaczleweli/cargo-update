#[cfg(all(windows, target_env = "msvc"))]
extern crate winreg;

use std::process::Command;
use std::path::{Path, PathBuf};
use std::env;

#[cfg(not(windows))]
fn main() { }

#[cfg(windows)]
fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    compile_resource(&out_dir);
    println!("cargo:rustc-link-search=native={}", out_dir);
}

#[cfg(all(windows, target_env = "msvc"))]
fn compile_resource(out_dir: &str) {
    // `.res`es are linkable under MSVC as well as normal libraries.

    fn find_windows_sdk_bin_dir() -> Option<PathBuf> {
        use winreg::enums::*;

        #[derive(Clone, Copy)]
        enum Arch {
            X86,
            X64,
        }

        // Windows 8 - 10
        fn find_windows_kits_bin_dir(key: &str, arch: Arch) -> Option<PathBuf> {
            let res = winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags("SOFTWARE\\Microsoft\\Windows Kits\\Installed Roots", KEY_QUERY_VALUE)
                .and_then(|reg_key| reg_key.get_value::<String, _>(key));

            match res {
                Ok(root_dir) => {
                    let mut p = PathBuf::from(root_dir);
                    match arch {
                        Arch::X86 => p.push("bin\\x86"),
                        Arch::X64 => p.push("bin\\x64"),
                    }
                    if p.is_dir() { Some(p) } else { None }
                }
                Err(_) => None
            }
        }

        // Windows Vista - 7
        fn find_latest_windows_sdk_bin_dir(arch: Arch) -> Option<PathBuf> {
            let res = winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags("SOFTWARE\\Microsoft\\Microsoft SDKs\\Windows", KEY_QUERY_VALUE)
                .and_then(|reg_key| reg_key.get_value::<String, _>("CurrentInstallFolder"));

            match res {
                Ok(root_dir) => {
                    let mut p = PathBuf::from(root_dir);
                    match arch {
                        Arch::X86 => p.push("Bin"),
                        Arch::X64 => p.push("Bin\\x64"),
                    }
                    if p.is_dir() { Some(p) } else { None }
                }
                Err(_) => None
            }
        }

        let arch = if env::var("TARGET").unwrap().starts_with("x86_64") { Arch::X64 } else { Arch::X86 };

        find_windows_kits_bin_dir("KitsRoot10", arch)
            .or_else(|| find_windows_kits_bin_dir("KitsRoot81", arch))
            .or_else(|| find_windows_kits_bin_dir("KitsRoot", arch))
            .or_else(|| find_latest_windows_sdk_bin_dir(arch))
    }

    let rc_exe = Path::new("rc.exe");
    let rc_path = find_windows_sdk_bin_dir()
        .and_then(|mut x| {
            x.push(rc_exe);
            if x.exists() { Some(x) } else { None }
        });

    Command::new(if let Some(ref x) = rc_path { x } else { rc_exe })
        .args(&["/fo", &format!("{}/cargo-install-update-manifest.lib", out_dir), "cargo-install-update-manifest.rc"])
        .status()
        .expect("Are you sure you have RC.EXE in your $PATH?");
}

#[cfg(all(windows, not(target_env = "msvc")))]
fn compile_resource(out_dir: &str) {
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
