use cargo_update::ops::{PackageFilterElement, PackageFilterElementValue};


#[test]
fn toolchain() {
    assert_eq!(PackageFilterElement::parse("toolchain=nightly"),
               Ok(PackageFilterElement(false, PackageFilterElementValue::Toolchain("nightly".to_string()))));
    assert_eq!(PackageFilterElement::parse("!toolchain=nightly"),
               Ok(PackageFilterElement(true, PackageFilterElementValue::Toolchain("nightly".to_string()))));
}


#[test]
fn name() {
    assert_eq!(PackageFilterElement::parse("name=cargo-update"),
               Ok(PackageFilterElement(false, PackageFilterElementValue::Name(["cargo-update"].map(str::to_string).to_vec()))));
    assert_eq!(PackageFilterElement::parse("name=cargo-*"),
               Ok(PackageFilterElement(false, PackageFilterElementValue::Name(["cargo-", ""].map(str::to_string).to_vec()))));
    assert_eq!(PackageFilterElement::parse("name=*update"),
               Ok(PackageFilterElement(false, PackageFilterElementValue::Name(["", "update"].map(str::to_string).to_vec()))));
    assert_eq!(PackageFilterElement::parse("name=*upd*"),
               Ok(PackageFilterElement(false, PackageFilterElementValue::Name(["", "upd", ""].map(str::to_string).to_vec()))));
    assert_eq!(PackageFilterElement::parse("name=*u*d*"),
               Ok(PackageFilterElement(false, PackageFilterElementValue::Name(["", "u", "d", ""].map(str::to_string).to_vec()))));
}
