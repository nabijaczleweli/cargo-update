use cargo_update::ops::crates_token;
use std::fs::{self, File};
use std::env::temp_dir;
use std::io::Write;


static CONFIG: &'static [u8] = include_bytes!("../../test-data/cargo-config.toml");


#[test]
fn existant() {
    let mut td = temp_dir();
    let _ = fs::create_dir(&td);
    td.push("cargo_update-test");
    let _ = fs::create_dir(&td);
    td.push("crates_token-existant");
    let _ = fs::create_dir(&td);

    File::create(td.join("config")).unwrap().write_all(CONFIG).unwrap();

    assert_eq!(crates_token(&td), Ok("Da39A3Ee5e6B4B0D3255bfeF95601890".to_string()));
}

#[test]
fn non_existant() {
    let mut td = temp_dir();
    let _ = fs::create_dir(&td);
    td.push("cargo_update-test");
    let _ = fs::create_dir(&td);
    td.push("crates_token-nonexistant");
    let _ = fs::create_dir(&td);

    assert_eq!(crates_token(&td), Err(-1));
}
