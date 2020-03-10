use cargo_update::ops::get_index_url;
use std::path::PathBuf;
use std::env::temp_dir;
use std::fs;


static TEST_DATA: &[u8] = include_bytes!("../../test-data/cargo.config");


#[test]
fn default() {
    assert_eq!(get_index_url(&prep_config("default"), "https://github.com/rust-lang/crates.io-index"),
               Ok("outside-the-scope-of-this-document".to_string()));
}

#[test]
fn from_alt_url() {
    assert_eq!(get_index_url(&prep_config("from_alt_url"), "file:///usr/local/share/cargo"),
               Ok("outside-the-scope-of-this-document".to_string()));
}

#[test]
fn from_name() {
    assert_eq!(get_index_url(&prep_config("from_name"), "alternative"),
               Ok("outside-the-scope-of-this-document".to_string()));
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
