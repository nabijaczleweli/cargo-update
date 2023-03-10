use cargo_update::ops::get_index_url;
use std::path::PathBuf;
use std::env::temp_dir;
use std::fs;


static TEST_DATA: &[u8] = include_bytes!("../../test-data/cargo.config");


#[test]
fn default_vs_sparse() {
    for suffix in &["config", "config.toml"] {
        let crates_file = prep_config("default_vs_sparse", suffix);
        fs::remove_file(crates_file.with_file_name(suffix)).unwrap();

        assert_eq!(get_index_url(&crates_file, "https://github.com/rust-lang/crates.io-index", false),
            Ok(("https://github.com/rust-lang/crates.io-index".to_string(), false, "crates-io".into())));
        assert_eq!(get_index_url(&crates_file, "https://github.com/rust-lang/crates.io-index", true),
            Ok(("https://index.crates.io/".to_string(), true, "crates-io".into())));
    }
}

#[test]
fn nonexistent() {
    for suffix in &["config", "config.toml"] {
        let crates_file = prep_config("nonexistent", suffix);
        fs::remove_file(crates_file.with_file_name(suffix)).unwrap();

        assert_eq!(get_index_url(&crates_file, "https://github.com/LoungeCPP/pir-8-emu", false),
                   Err(format!("Non-crates.io registry specified and no config file found at {} or {}. Due to a Cargo limitation we will not be able to \
                                install from there until it's given a [source.NAME] in that file!",
                               crates_file.with_file_name("config").display(),
                               crates_file.with_file_name("config.toml").display())
                       .into()));
    }
}

#[test]
fn unknown() {
    for suffix in &["config", "config.toml"] {
        let crates_file = prep_config("unknown", suffix);
        assert_eq!(get_index_url(&crates_file, "https://github.com/LoungeCPP/pir-8-emu", false),
                   Err(format!("Non-crates.io registry specified and https://github.com/LoungeCPP/pir-8-emu couldn't be found in the config file at {}. \
                                Due to a Cargo limitation we will not be able to install from there until it's given a [source.NAME] in that file!",
                               crates_file.with_file_name(suffix).display())
                       .into()));
    }
}

#[test]
fn default() {
    for suffix in &["config", "config.toml"] {
        assert_eq!(get_index_url(&prep_config("default", suffix), "https://github.com/rust-lang/crates.io-index", false),
                   Ok(("outside-the-scope-of-this-document".to_string(), false, "tralternative".into())));
    }
}

#[test]
fn from_alt_url() {
    for suffix in &["config", "config.toml"] {
        assert_eq!(get_index_url(&prep_config("from_alt_url", suffix), "file:///usr/local/share/cargo", false),
                   Ok(("outside-the-scope-of-this-document".to_string(), false, "tralternative".into())));
    }
}

#[test]
fn from_name() {
    for suffix in &["config", "config.toml"] {
        assert_eq!(get_index_url(&prep_config("from_name", suffix), "alternative", false),
                   Ok(("outside-the-scope-of-this-document".to_string(), false, "tralternative".into())));
    }
}

#[test]
fn sus() {
    for suffix in &["config", "config.toml"] {
        assert_eq!(get_index_url(&prep_config("sus", suffix), "sus", false),
                   Ok(("zupa".to_string(), true, "sussy".into())));
    }
}

#[test]
fn dead_end() {
    for suffix in &["config", "config.toml"] {
        let crates_file = prep_config("dead_end", suffix);
        assert_eq!(get_index_url(&crates_file, "dead-end", false),
                   Err(format!("Couldn't find appropriate source URL for dead-end in {} (resolved to \"death\")",
                               crates_file.with_file_name(suffix).display())
                       .into()));
    }
}


fn prep_config(subname: &str, suffix: &str) -> PathBuf {
    let td = temp_dir().join("cargo_update-test").join(format!("get_index_url-{}-{}", subname, suffix));
    let _ = fs::create_dir_all(&td);

    fs::write(td.join(suffix), TEST_DATA).unwrap();
    td.join(".crates.toml")
}
