use cargo_update::ops::{self, MainRepoPackage};
use semver::Version as Semver;
use std::fs::File;

mod installed_main_repo_packages;
mod main_repo_package;
mod get_index_path;


#[test]
fn intersect_packages() {
    assert_eq!(ops::intersect_packages(vec![MainRepoPackage::parse("cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
                                            MainRepoPackage::parse("cargo-count 0.2.2 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
                                            MainRepoPackage::parse("racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)").unwrap()],
                                       &[("cargo-count".to_string(), None), ("racer".to_string(), None), ("checksums".to_string(), None)],
                                       false),
               vec![MainRepoPackage::parse("cargo-count 0.2.2 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
                    MainRepoPackage::parse("racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)").unwrap()]);
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
