use cargo_update::ops::PackageFilterElement;


#[test]
fn toolchain() {
    assert_eq!(PackageFilterElement::parse("toolchain=nightly"),
               Ok(PackageFilterElement::Toolchain("nightly".to_string())));
}
