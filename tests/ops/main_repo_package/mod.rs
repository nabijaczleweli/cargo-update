mod parse;

use cargo_update::ops::MainRepoPackage;
use semver::Version as Semver;
use std::env;
use std::fs;


#[test]
fn pull_version() {
    let mut td = env::temp_dir();
    for chunk in &["cargo_update-test", "MainRepoPackage-pull_version", "registry", "index", "github.com-1ecc6299db9ec823"] {
        td.push(chunk);
        let _ = fs::create_dir(&td);
    }
    {
        let mut td = td.clone();
        for chunk in &["ch", "ec"] {
            td.push(chunk);
            let _ = fs::create_dir(&td);
        }
        fs::copy("test-data/checksums-versions.json", td.join("checksums")).unwrap();
    }

    let mut pkg = MainRepoPackage::parse("checksums 0.5.0 (registry+https://github.com/rust-lang/crates.io-index)").unwrap();
    pkg.pull_version(&td);
    assert_eq!(pkg.newest_version, Some(Semver::parse("0.5.2").unwrap()));
}
