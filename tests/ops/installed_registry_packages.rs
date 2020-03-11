use cargo_update::ops::{RegistryPackage, installed_registry_packages};
use semver::Version as Semver;
use std::fs::{self, File};
use std::env::temp_dir;
use std::io::Write;


static CRATES: &[u8] = include_bytes!("../../test-data/.cargo-crates.toml");


#[test]
fn existent() {
    let mut td = temp_dir().join("cargo_update-test").join("installed_registry_packages-existent");
    let _ = fs::create_dir_all(&td);
    td.push(".crates.toml");

    File::create(&td).unwrap().write_all(CRATES).unwrap();

    assert_eq!(installed_registry_packages(&td),
               vec![RegistryPackage {
                        name: "cargo-outdated".to_string(),
                        registry: "https://github.com/rust-lang/crates.io-index".to_string(),
                        version: Some(Semver::parse("0.2.0").unwrap()),
                        newest_version: None,
                        alternative_version: None,
                        max_version: None,
                    },
                    RegistryPackage {
                        name: "racer".to_string(),
                        registry: "https://github.com/rust-lang/crates.io-index".to_string(),
                        version: Some(Semver::parse("1.2.10").unwrap()),
                        newest_version: None,
                        alternative_version: None,
                        max_version: None,
                    },
                    RegistryPackage {
                        name: "rustfmt".to_string(),
                        registry: "file:///usr/local/share/cargo".to_string(),
                        version: Some(Semver::parse("0.6.2").unwrap()),
                        newest_version: None,
                        alternative_version: None,
                        max_version: None,
                    }]);
}

#[test]
fn non_existent() {
    let td = temp_dir().join("cargo_update-test").join("installed_registry_packages-nonexistent");
    let _ = fs::create_dir_all(&td);

    assert_eq!(installed_registry_packages(&td.join(".crates.toml")), vec![]);
}
