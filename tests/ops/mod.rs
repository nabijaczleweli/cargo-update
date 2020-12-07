use cargo_update::ops::{self, RegistryPackage};
use semver::Version as Semver;
use std::fs::File;

mod installed_registry_packages;
mod package_filter_element;
mod assert_index_path;
mod registry_package;
mod get_index_url;


#[test]
fn intersect_packages() {
    assert_eq!(ops::intersect_packages(&[RegistryPackage::parse("cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
                                         RegistryPackage::parse("cargo-count 0.2.2 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
                                         RegistryPackage::parse("racer 1.2.10 (registry+file:///usr/local/share/cargo)").unwrap()],
                                       &[("cargo-count".to_string(), None, "https://github.com/rust-lang/crates.io-index".to_string()),
                                         ("racer".to_string(), None, "https://github.com/rust-lang/crates.io-index".to_string()),
                                         ("checksums".to_string(), None, "file:///usr/local/share/cargo".to_string())],
                                       false,
                                       &[]),
               vec![RegistryPackage::parse("cargo-count 0.2.2 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
                    RegistryPackage::parse("racer 1.2.10 (registry+file:///usr/local/share/cargo)").unwrap()]);
}

#[test]
fn crate_versions() {
    assert_eq!(ops::crate_versions(&mut File::open("test-data/checksums-versions.json").unwrap()),
               vec![Semver::parse("0.2.0").unwrap(),
                    Semver::parse("0.2.1").unwrap(),
                    Semver::parse("0.3.0").unwrap(),
                    Semver::parse("0.4.0").unwrap(),
                    Semver::parse("0.4.1").unwrap(),
                    Semver::parse("0.5.0").unwrap(),
                    Semver::parse("0.5.1").unwrap(),
                    Semver::parse("0.5.2").unwrap()]);
}
