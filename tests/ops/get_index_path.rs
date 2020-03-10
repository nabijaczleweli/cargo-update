use cargo_update::ops::get_index_path;
use std::path::{PathBuf, Path};
use std::fs::{self, File};
use std::env::temp_dir;


#[test]
fn nonexistant() {
    let indices = prep_indices("nonexistant");

    assert_eq!(get_index_path(&indices, "https://github.com/rust-lang/crates.io-index"),
               Err(format!(r"Couldn't read {} (index directory for https://github.com/rust-lang/crates.io-index): {}",
                           indices.join("registry").join("index").join("github.com-1ecc6299db9ec823").display(),
                           "The system cannot find the path specified. (os error 3)")
                   .into()));
}

#[test]
fn is_file() {
    let indices = prep_indices("is_file");

    prepare_indices(&indices, &[]);
    File::create(indices.join("registry").join("index").join("github.com-1ecc6299db9ec823")).unwrap();

    assert_eq!(get_index_path(&indices, "https://github.com/rust-lang/crates.io-index"),
               Err(format!("{} (index directory for https://github.com/rust-lang/crates.io-index) not a directory",
                           indices.join("registry").join("index").join("github.com-1ecc6299db9ec823").display())
                   .into()));
}

#[test]
fn single() {
    let indices = prep_indices("single");

    prepare_indices(&indices, &[("github.com", "1ecc6299db9ec823")]);

    assert_eq!(get_index_path(&indices, "https://github.com/rust-lang/crates.io-index"),
               Ok(indices.join("registry").join("index").join("github.com-1ecc6299db9ec823")));
}

#[test]
fn double() {
    let indices = prep_indices("double");

    prepare_indices(&indices, &[("github.com", "1ecc6299db9ec823"), ("github.com", "48ad6e4054423464")]);

    assert_eq!(get_index_path(&indices, "https://github.com/rust-lang/crates.io-index"),
               Ok(indices.join("registry").join("index").join("github.com-1ecc6299db9ec823")));
}

#[test]
fn two() {
    let indices = prep_indices("two");

    prepare_indices(&indices,
                    &[("github.com", "1ecc6299db9ec823"), ("", "72ffea3e1e10b7e3"), ("github.com", "48ad6e4054423464")]);

    assert_eq!(get_index_path(&indices, "https://github.com/rust-lang/crates.io-index"),
               Ok(indices.join("registry").join("index").join("github.com-1ecc6299db9ec823")));

    assert_eq!(get_index_path(&indices, "file:///usr/local/share/cargo"),
               Ok(indices.join("registry").join("index").join("-72ffea3e1e10b7e3")));
}

fn prep_indices(subname: &str) -> PathBuf {
    let td = temp_dir().join("cargo_update-test").join(format!("get_index_path-{}", subname));
    let _ = fs::create_dir_all(&td);
    td
}

fn prepare_indices(index: &Path, shortnames: &[(&str, &str)]) {
    let index = index.join("registry").join("index");
    let _ = fs::create_dir_all(&index);

    for (name, hash) in shortnames {
        let _ = fs::create_dir(index.join(format!("{}-{}", name, hash)));
    }
}
