use cargo_update::ops::{MainRepoPackage, installed_main_repo_packages};
use semver::Version as Semver;
use std::fs::{self, File};
use std::env::temp_dir;
use std::io::Write;


static CRATES: &'static [u8] = include_bytes!("../../test-data/.cargo-crates.toml");


#[test]
fn existant() {
    let mut td = temp_dir();
    let _ = fs::create_dir(&td);
    td.push("cargo_update-test");
    let _ = fs::create_dir(&td);
    td.push("installed_main_repo_packages-existant");
    let _ = fs::create_dir(&td);
    td.push(".crates.toml");

    File::create(&td).unwrap().write_all(CRATES).unwrap();

    assert_eq!(installed_main_repo_packages(&td),
               vec![MainRepoPackage {
                        name: "cargo-outdated".to_string(),
                        version: Some(Semver::parse("0.2.0").unwrap()),
                        newest_version: None,
                        max_version: None,
                    },
                    MainRepoPackage {
                        name: "racer".to_string(),
                        version: Some(Semver::parse("1.2.10").unwrap()),
                        newest_version: None,
                        max_version: None,
                    },
                    MainRepoPackage {
                        name: "rustfmt".to_string(),
                        version: Some(Semver::parse("0.6.2").unwrap()),
                        newest_version: None,
                        max_version: None,
                    }]);
}

#[test]
fn non_existant() {
    let mut td = temp_dir();
    let _ = fs::create_dir(&td);
    td.push("cargo_update-test");
    let _ = fs::create_dir(&td);
    td.push("installed_main_repo_packages-nonexistant");
    let _ = fs::create_dir(&td);
    td.push(".crates.toml");

    assert_eq!(installed_main_repo_packages(&td), vec![]);
}
