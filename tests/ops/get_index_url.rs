use cargo_update::ops::get_index_url;
use std::path::PathBuf;
use std::env::temp_dir;
use std::fs;


static TEST_DATA: &[u8] = include_bytes!("../../test-data/cargo.config");


#[test]
fn nonexistant() {
    let crates_file = prep_config("nonexistant");
    fs::remove_file(crates_file.with_file_name("config")).unwrap();

    assert_eq!(get_index_url(&crates_file, "https://github.com/LoungeCPP/pir-8-emu"),
               Err(format!("Non-crates.io registry specified and no config file found at {}. \
                            Due to a Cargo limitation we will not be able to install from there \
                            until it's given a [source.NAME] in that file!",
                           crates_file.with_file_name("config").display())
                   .into()));
}

#[test]
fn unknown() {
    let crates_file = prep_config("unknown");
    assert_eq!(get_index_url(&crates_file, "https://github.com/LoungeCPP/pir-8-emu"),
               Err(format!("Non-crates.io registry specified and https://github.com/LoungeCPP/pir-8-emu couldn't be found in the config file at {}. \
                            Due to a Cargo limitation we will not be able to install from there \
                            until it's given a [source.NAME] in that file!",
                           crates_file.with_file_name("config").display())
                   .into()));
}

#[test]
fn default() {
    assert_eq!(get_index_url(&prep_config("default"), "https://github.com/rust-lang/crates.io-index"),
               Ok(("outside-the-scope-of-this-document".to_string(), "tralternative".into())));
}

#[test]
fn from_alt_url() {
    assert_eq!(get_index_url(&prep_config("from_alt_url"), "file:///usr/local/share/cargo"),
               Ok(("outside-the-scope-of-this-document".to_string(), "tralternative".into())));
}

#[test]
fn from_name() {
    assert_eq!(get_index_url(&prep_config("from_name"), "alternative"),
               Ok(("outside-the-scope-of-this-document".to_string(), "tralternative".into())));
}

#[test]
fn dead_end() {
    let crates_file = prep_config("dead_end");
    assert_eq!(get_index_url(&crates_file, "dead-end"),
               Err(format!("Couldn't find appropriate source URL for dead-end in {} (resolved to \"death\")",
                           crates_file.with_file_name("config").display())
                   .into()));
}


fn prep_config(subname: &str) -> PathBuf {
    let td = temp_dir().join("cargo_update-test").join(format!("get_index_url-{}", subname));
    let _ = fs::create_dir_all(&td);

    fs::write(td.join("config"), TEST_DATA).unwrap();
    td.join(".crates.toml")
}
