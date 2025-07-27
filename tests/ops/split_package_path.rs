use cargo_update::ops;


#[test]
#[should_panic = "0-length cratename"]
fn empty() {
    ops::split_package_path("");
}

#[test]
fn normal() {
    assert_eq!(ops::split_package_path("a"), vec!["1", "a"]);
    assert_eq!(ops::split_package_path("an"), vec!["2", "an"]);
    assert_eq!(ops::split_package_path("jot"), vec!["3", "j", "jot"]);
    assert_eq!(ops::split_package_path("cargo-update"), vec!["ca", "rg", "cargo-update"]);

    assert_eq!(ops::split_package_path("FileSorterX"), vec!["fi", "le", "filesorterx"]);
}
