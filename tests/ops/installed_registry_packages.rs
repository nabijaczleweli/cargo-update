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
                        executables: vec!["cargo-outdated.exe".to_string()],
                    },
                    RegistryPackage {
                        name: "racer".to_string(),
                        registry: "https://github.com/rust-lang/crates.io-index".to_string(),
                        version: Some(Semver::parse("1.2.10").unwrap()),
                        newest_version: None,
                        alternative_version: None,
                        max_version: None,
                        executables: vec!["racer.exe".to_string()],
                    },
                    RegistryPackage {
                        name: "rustfmt".to_string(),
                        registry: "file:///usr/local/share/cargo".to_string(),
                        version: Some(Semver::parse("0.6.2").unwrap()),
                        newest_version: None,
                        alternative_version: None,
                        max_version: None,
                        executables: vec!["cargo-fmt.exe".to_string(), "rustfmt.exe".to_string()],
                    }]);
}

#[test]
fn non_existent() {
    let td = temp_dir().join("cargo_update-test").join("installed_registry_packages-nonexistent");
    let _ = fs::create_dir_all(&td);

    assert_eq!(installed_registry_packages(&td.join(".crates.toml")), vec![]);
}

#[test]
fn with_private_index_package() {
    let mut td = temp_dir().join("cargo_update-test").join("installed_registry_packages-with-private-index");
    let _ = fs::create_dir_all(&td);
    td.push(".crates.toml");

    let test_data = r#"[v1]
"cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)" = ["cargo-outdated.exe"]
"treesize 0.2.1 (git+https://github.com/melak47/treesize-rs#742aebb3e66bd14421eb148e7f7981d50c6d1423)" = ["treesize.exe"]
"problematic-package 1.0.0 (registry+ssh://example.com/private-index)" = ["problematic-package.exe"]
"#;

    File::create(&td).unwrap().write_all(test_data.as_bytes()).unwrap();

    let packages = installed_registry_packages(&td);
    
    assert_eq!(packages.len(), 2);
    
    assert!(packages.iter().any(|p| p.name == "cargo-outdated"));
    assert!(packages.iter().any(|p| p.name == "problematic-package"));
    
    assert!(!packages.iter().any(|p| p.name == "treesize"));
    
    let problematic = packages.iter().find(|p| p.name == "problematic-package").unwrap();
    assert_eq!(problematic.registry, "ssh://example.com/private-index");
}

