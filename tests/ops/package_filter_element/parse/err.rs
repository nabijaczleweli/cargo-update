use cargo_update::ops::PackageFilterElement;


#[test]
fn no_separator() {
    assert_eq!(PackageFilterElement::parse("toolchain"),
               Err(r#"Filter string "toolchain" does not contain the key/value separator "=""#.to_string()));
}

#[test]
fn unrecognised() {
    assert_eq!(PackageFilterElement::parse("henlo=benlo"), Err(r#"Unrecognised filter key "henlo""#.to_string()));
}
