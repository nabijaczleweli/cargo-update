use cargo_update::ops::RegistryPackage;
use semver::Version as Semver;


#[test]
fn main_registry() {
    assert_eq!(RegistryPackage::parse("cargo-count 0.2.2 (registry+https://github.com/rust-lang/crates.io-index)"),
               Some(RegistryPackage {
                   name: "cargo-count".to_string(),
                   registry_url: "https://github.com/rust-lang/crates.io-index".to_string(),
                   version: Some(Semver::parse("0.2.2").unwrap()),
                   newest_version: None,
                   alternative_version: None,
                   max_version: None,
               }));
}

#[test]
fn alt_registry() {
    assert_eq!(RegistryPackage::parse("cargo-count 0.2.2 (registry+file:///usr/local/share/cargo)"),
               Some(RegistryPackage {
                   name: "cargo-count".to_string(),
                   registry_url: "file:///usr/local/share/cargo".to_string(),
                   version: Some(Semver::parse("0.2.2").unwrap()),
                   newest_version: None,
                   alternative_version: None,
                   max_version: None,
               }));
}

#[test]
fn git() {
    assert_eq!(RegistryPackage::parse("treesize 0.2.1 (git+https://github.com/melak47/treesize-rs#742aebb3e66bd14421eb148e7f7981d50c6d1423)"),
               None);
}

#[test]
fn invalid() {
    assert_eq!(RegistryPackage::parse("treesize 0.2.1 (gi"), None);
}
