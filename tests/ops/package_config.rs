use cargo_update::ops::PackageConfig;
use std::env::temp_dir;
use std::fs::{self, File};


static CRATES2: &[u8] = include_bytes!("../../test-data/cargo.crates2.json");


#[test]
fn read() {
    let td = temp_dir().join("cargo_update-test").join("package_config-read");
    let _ = fs::create_dir_all(&td);

    let cfg = td.join(".install_config.toml");
    File::create(&cfg).unwrap();

    let crates2 = td.join(".crates2.json");
    fs::write(&crates2, CRATES2).unwrap();

    let dfl = PackageConfig { from_transient: true, ..Default::default() };
    assert_eq!(PackageConfig::read(&cfg, &crates2),
               Ok(vec![("bindgen-cli".to_string(), PackageConfig { default_features: false, ..dfl.clone() }),
                       ("cargo-audit".to_string(), PackageConfig { build_profile: Some("test".into()), ..dfl.clone() }),
                       ("cargo-bisect-rustc".to_string(), PackageConfig { ..dfl.clone() }),
                       ("cargo-count".to_string(), PackageConfig { ..dfl.clone() }),
                       ("cargo-deb".to_string(), PackageConfig { ..dfl.clone() }),
                       ("cargo-graph".to_string(), PackageConfig { ..dfl.clone() }),
                       ("cargo-navigate".to_string(), PackageConfig { ..dfl.clone() }),
                       ("cargo-outdated".to_string(), PackageConfig { ..dfl.clone() }),
                       ("cargo-update".to_string(), PackageConfig { ..dfl.clone() }),
                       ("checksums".to_string(), PackageConfig { ..dfl.clone() }),
                       ("gen-epub-book".to_string(), PackageConfig { ..dfl.clone() }),
                       ("https".to_string(), PackageConfig { ..dfl.clone() }),
                       ("identicon".to_string(), PackageConfig { ..dfl.clone() }),
                       ("racer".to_string(), PackageConfig { ..dfl.clone() }),
                       ("ripgrep".to_string(), PackageConfig { features: vec!["dupowiec".to_string()].into_iter().collect(), ..dfl.clone() }),
                       ("rustup-toolchain-install-master".to_string(), PackageConfig { ..dfl.clone() }),
                       ("tauri-cli".to_string(), PackageConfig { ..dfl.clone() }),
                       ("treesize".to_string(), PackageConfig { ..dfl.clone() })]
                   .into_iter()
                   .collect()));
}
