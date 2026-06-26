use cargo_update::ops::{PackageFilterElement, PackageFilterElementValue};


#[test]
fn toolchain() {
    assert_eq!(PackageFilterElement::parse("toolchain=nightly"),
               Ok(PackageFilterElement(false, PackageFilterElementValue::Toolchain("nightly".to_string()))));
    assert_eq!(PackageFilterElement::parse("!toolchain=nightly"),
               Ok(PackageFilterElement(true, PackageFilterElementValue::Toolchain("nightly".to_string()))));
}
