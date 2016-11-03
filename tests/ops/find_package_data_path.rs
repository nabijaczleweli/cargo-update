use cargo_update::ops::find_package_data_path;
use std::path::{PathBuf, Path};
use std::fs::{self, File};
use std::env::temp_dir;


#[test]
#[should_panic]
fn zero_length() {
    find_package_data_path("", &prep_index("zero_length"));
}

#[test]
fn one_length() {
    let index = prep_index("one_length");

    add_package(&index, "1", "a");

    assert_eq!(find_package_data_path("a", &index), Some(index.join("1").join("a")));
    assert_eq!(find_package_data_path("b", &index), None);
}

#[test]
fn two_length() {
    let index = prep_index("two_length");

    add_package(&index, "2", "ab");

    assert_eq!(find_package_data_path("ab", &index), Some(index.join("2").join("ab")));
    assert_eq!(find_package_data_path("bc", &index), None);
}

#[test]
fn three_length() {
    let index = prep_index("three_length");

    add_package(&index, "3/a", "abc");

    assert_eq!(find_package_data_path("abc", &index), Some(index.join("3").join("a").join("abc")));
    assert_eq!(find_package_data_path("abe", &index), None);
    assert_eq!(find_package_data_path("zxt", &index), None);
}

#[test]
fn four_length() {
    let index = prep_index("four_length");

    add_package(&index, "ab/cd", "abcd");

    assert_eq!(find_package_data_path("abcd", &index), Some(index.join("ab").join("cd").join("abcd")));
    assert_eq!(find_package_data_path("zxth", &index), None);
}

#[test]
fn five_length() {
    let index = prep_index("five_length");

    add_package(&index, "ab/cd", "abcde");

    assert_eq!(find_package_data_path("abcde", &index), Some(index.join("ab").join("cd").join("abcde")));
    assert_eq!(find_package_data_path("zxthk", &index), None);
}

#[test]
fn more_length() {
    let index = prep_index("more_length");

    add_package(&index, "ca/rg", "cargo-update");

    assert_eq!(find_package_data_path("cargo-update", &index),
               Some(index.join("ca").join("rg").join("cargo-update")));
    assert_eq!(find_package_data_path("sieg-heil", &index), None);
}

fn prep_index(subname: &str) -> PathBuf {
    let mut td = temp_dir();
    let _ = fs::create_dir(&td);
    td.push("cargo_update-test");
    let _ = fs::create_dir(&td);
    td.push(format!("find_package_data_path-{}", subname));
    let _ = fs::create_dir(&td);
    td
}

fn add_package(index: &Path, subpath: &str, name: &str) {
    File::create(subpath.split('/')
            .fold(index.to_path_buf(), |mut idx, elem| {
                idx.push(elem);
                let _ = fs::create_dir(&idx);
                idx
            })
            .join(name))
        .unwrap();
}
