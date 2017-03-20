extern crate cargo_update;

use std::process::exit;
use std::collections::BTreeMap;


fn main() {
    let result = actual_main().err().unwrap_or(0);
    exit(result);
}

fn actual_main() -> Result<(), i32> {
    let opts = cargo_update::ConfigOptions::parse();
    println!("{:#?}", opts);
    let config_file = cargo_update::ops::resolve_crates_file(opts.crates_file.1).with_file_name(".install_config.toml");
    println!("{}", config_file.display());

    let mut mep = BTreeMap::new();
    mep.insert("cargo-update".to_string(),
               cargo_update::ops::PackageConfig {
                   toolchain: None,
                   default_features: true,
                   features: vec!["capitalism".to_string()],
               });
    mep.insert("bear-lib-terminal".to_string(),
               cargo_update::ops::PackageConfig {
                   toolchain: Some("nightly".to_string()),
                   default_features: false,
                   features: vec!["capitalism".to_string(), "exhuberance".to_string()],
               });
    try!(cargo_update::ops::PackageConfig::write(&mep, &config_file));

    Ok(())
}
