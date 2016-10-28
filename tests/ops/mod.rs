use cargo_update::ops::{self, MainRepoPackage};
use semver::Version as Semver;

mod installed_main_repo_packages;
mod main_repo_package;
mod crates_token;


static CHECKSUMS_VERSIONS: &'static str = include_str!("../../test-data/checksums-versions.json");


#[test]
fn intersect_packages() {
    assert_eq!(ops::intersect_packages(vec![MainRepoPackage::parse("cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
                                            MainRepoPackage::parse("cargo-count 0.2.2 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
                                            MainRepoPackage::parse("racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)").unwrap()],
                                       &["cargo-count".to_string(), "racer".to_string(), "checksums".to_string()]),
               vec![MainRepoPackage::parse("cargo-count 0.2.2 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
                    MainRepoPackage::parse("racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)").unwrap()]);
}

#[test]
fn crate_versions() {
    assert_eq!(ops::crate_versions(CHECKSUMS_VERSIONS),
               vec![Semver::parse("0.5.2").unwrap(),
                    Semver::parse("0.5.1").unwrap(),
                    Semver::parse("0.5.0").unwrap(),
                    Semver::parse("0.4.1").unwrap(),
                    Semver::parse("0.4.0").unwrap(),
                    Semver::parse("0.3.0").unwrap(),
                    Semver::parse("0.2.1").unwrap(),
                    Semver::parse("0.2.0").unwrap(),
                    Semver::parse("0.1.1").unwrap(),
                    Semver::parse("0.1.0").unwrap()]);
}
