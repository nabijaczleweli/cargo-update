use cargo_update::ops::get_index_path;
use std::path::{PathBuf, Path};
use std::fs::{self, File};
use std::time::Duration;
use std::thread::sleep;
use std::env::temp_dir;


#[test]
fn nonexistant() {
    let indices = prep_indices("nonexistant");

    assert_eq!(get_index_path(&indices, None), Err("index directory nonexistant"));
}

#[test]
fn empty() {
    let indices = prep_indices("empty");

    prepare_indices(&indices, &[]);

    assert_eq!(get_index_path(&indices, None), Err("empty index directory"));
}

#[test]
fn single() {
    let indices = prep_indices("single");

    prepare_indices(&indices, &["1ecc6299db9ec823"]);

    assert_eq!(get_index_path(&indices, None),
               Ok(indices.join("registry").join("index").join("github.com-1ecc6299db9ec823")));
}

#[test]
fn double() {
    let indices = prep_indices("double");

    prepare_indices(&indices, &["1ecc6299db9ec823", "48ad6e4054423464"]);

    assert_eq!(get_index_path(&indices, None),
               Ok(indices.join("registry").join("index").join("github.com-48ad6e4054423464")));
}

#[test]
fn triple() {
    let indices = prep_indices("triple");

    prepare_indices(&indices, &["1ecc6299db9ec823", "88ac128001ac3a9a", "48ad6e4054423464"]);

    assert_eq!(get_index_path(&indices, None),
               Ok(indices.join("registry").join("index").join("github.com-48ad6e4054423464")));
}

#[test]
fn with_file() {
    let indices = prep_indices("with_file");

    prepare_indices(&indices, &["1ecc6299db9ec823", "88ac128001ac3a9a"]);
    File::create(indices.join("registry").join("index").join("I-am-a-random-file-yes")).unwrap();

    assert_eq!(get_index_path(&indices, None),
               Ok(indices.join("registry").join("index").join("github.com-88ac128001ac3a9a")));
}

fn prep_indices(subname: &str) -> PathBuf {
    let mut td = temp_dir();
    let _ = fs::create_dir(&td);
    td.push("cargo_update-test");
    let _ = fs::create_dir(&td);
    td.push(format!("get_index_path-{}", subname));
    let _ = fs::create_dir(&td);
    td
}

fn prepare_indices(index: &Path, hashes: &[&str]) {
    let mut index = index.to_path_buf();
    index.push("registry");
    let _ = fs::create_dir(&index);
    index.push("index");
    let _ = fs::create_dir(&index);

    for hash in hashes {
        let _ = fs::create_dir(index.join(format!("github.com-{}", hash)));
        sleep(Duration::from_millis(10));
    }
}
