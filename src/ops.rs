//! Main functions doing actual work.
//!
//! Use `installed_main_repo_packages()` to list the installed packages,
//! then use `intersect_packages()` to confirm which ones should be updated,
//! poll the packages' latest versions by calling `MainRepoPackage::pull_version` on them,
//! continue with doing whatever you wish.


use hyper::Client as HttpClient;
use semver::Version as Semver;
use std::path::Path;
use std::fs::File;
use std::io::Read;
use regex::Regex;
use toml;
use json;


lazy_static! {
    static ref PACKAGE_RGX: Regex = Regex::new(r"([^\s]+) ([^\s]+) \(([^+\s]+)+\+([^\s]+)\)").unwrap();
}


/// A representation of a package from the main [`crates.io`](https://crates.io) repository.
///
/// The newest version of a package is pulled from [`crates.io`](https://crates.io) via `pull_version()`.
///
/// The `parse()` function parses the format used in `$HOME/.cargo/.crates.toml`.
///
/// # Examples
///
/// ```
/// # extern crate cargo_update;
/// # extern crate semver;
/// # use cargo_update::ops::MainRepoPackage;
/// # use semver::Version as Semver;
/// # fn main() {
/// let package_s = "racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)";
/// let mut package = MainRepoPackage::parse(package_s).unwrap();
/// assert_eq!(package,
///            MainRepoPackage {
///                name: "racer".to_string(),
///                version: Semver::parse("1.2.10").unwrap(),
///                newest_version: None,
///            });
///
/// package.pull_version();
/// assert!(package.newest_version.is_some());
/// # }
/// ```
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct MainRepoPackage {
    /// The package's name.
    ///
    /// Go to `https://crates.io/crates/{name}` to get the crate info.
    pub name: String,
    /// The package's locally installed version.
    pub version: Semver,
    /// The latest version of the package vailable at the main [`crates.io`](https://crates.io) repository.
    ///
    /// `None` by default, acquire via `MainRepoPackage::pull_version()`.
    pub newest_version: Option<Semver>,
}

impl MainRepoPackage {
    /// Try to decypher a package descriptor into a `MainRepoPackage`.
    ///
    /// Will return `None` if:
    ///
    ///   * the given package descriptor is invalid, or
    ///   * the package descriptor is not from the main [`crates.io`](https://crates.io) registry.
    ///
    /// In the returned instance, `newest_version` is always `None`, get it via `MainRepoPackage::pull_version()`.
    ///
    /// # Examples
    ///
    /// Main repository packages:
    ///
    /// ```
    /// # extern crate cargo_update;
    /// # extern crate semver;
    /// # use cargo_update::ops::MainRepoPackage;
    /// # use semver::Version as Semver;
    /// # fn main() {
    /// let package_s = "racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)";
    /// assert_eq!(MainRepoPackage::parse(package_s).unwrap(),
    ///            MainRepoPackage {
    ///                name: "racer".to_string(),
    ///                version: Semver::parse("1.2.10").unwrap(),
    ///                newest_version: None,
    ///            });
    ///
    /// let package_s = "cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)";
    /// assert_eq!(MainRepoPackage::parse(package_s).unwrap(),
    ///            MainRepoPackage {
    ///                name: "cargo-outdated".to_string(),
    ///                version: Semver::parse("0.2.0").unwrap(),
    ///                newest_version: None,
    ///            });
    /// # }
    /// ```
    ///
    /// Git repository:
    ///
    /// ```
    /// # use cargo_update::ops::MainRepoPackage;
    /// let package_s = "treesize 0.2.1 (git+https://github.com/melak47/treesize-rs#v0.2.1)";
    /// assert!(MainRepoPackage::parse(package_s).is_none());
    /// ```
    pub fn parse(what: &str) -> Option<MainRepoPackage> {
        PACKAGE_RGX.captures(what).and_then(|c| if c.at(3).unwrap() == "registry" {
            Some(MainRepoPackage {
                name: c.at(1).unwrap().to_string(),
                version: Semver::parse(c.at(2).unwrap()).unwrap(),
                newest_version: None,
            })
        } else {
            None
        })
    }

    /// Download the version list for this crate off the main [`crates.io`](https://crates.io) registry.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cargo_update::ops::MainRepoPackage;
    /// let package_s = "racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)";
    /// let mut package = MainRepoPackage::parse(package_s).unwrap();
    /// package.pull_version();
    /// assert!(package.newest_version.is_some());
    /// ```
    pub fn pull_version(&mut self) {
        let vers = crate_versions(&crate_versions_raw(&self.name));
        self.newest_version = vers.into_iter().max();
    }
}


/// List the installed packages at the specified location that originate
/// from the main [`crates.io`](https://crates.io) registry.
///
/// If the `.crates.toml` file doesn't exist an empty vector is returned.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::installed_main_repo_packages;
/// # use std::env::temp_dir;
/// # let cargo_dir = temp_dir();
/// let packages = installed_main_repo_packages(&cargo_dir);
/// for package in &packages {
///     println!("{} v{}", package.name, package.version);
/// }
/// ```
pub fn installed_main_repo_packages(cargo_dir: &Path) -> Vec<MainRepoPackage> {
    let crates_path = cargo_dir.join(".crates.toml");
    if crates_path.exists() {
        let mut crates = String::new();
        File::open(crates_path).unwrap().read_to_string(&mut crates).unwrap();

        toml::Parser::new(&crates).parse().unwrap()["v1"].as_table().unwrap().keys().flat_map(|s| MainRepoPackage::parse(s)).collect()
    } else {
        Vec::new()
    }
}

/// Filter out the installed packages not specified to be updated.
///
/// List installed packages with `installed_main_repo_packages()`.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::{MainRepoPackage, intersect_packages};
/// # fn installed_main_repo_packages(_: &()) {}
/// # let cargo_dir = ();
/// # let packages_to_update = ["racer".to_string(), "cargo-outdated".to_string()];
/// let mut installed_packages = installed_main_repo_packages(&cargo_dir);
/// # let mut installed_packages =
/// #     vec![MainRepoPackage::parse("cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
/// #          MainRepoPackage::parse("racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
/// #          MainRepoPackage::parse("rustfmt 0.6.2 (registry+https://github.com/rust-lang/crates.io-index)").unwrap()];
/// installed_packages = intersect_packages(installed_packages, &packages_to_update);
/// # assert_eq!(&installed_packages,
/// #   &[MainRepoPackage::parse("cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
/// #     MainRepoPackage::parse("racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)").unwrap()]);
/// ```
pub fn intersect_packages(installed: Vec<MainRepoPackage>, to_update: &[String]) -> Vec<MainRepoPackage> {
    installed.into_iter().filter(|p| to_update.contains(&p.name)).collect()
}

/// Download the list of versions of the specified package from the main [`crates.io`](https://crates.io) registry
/// using the specified auth token.
///
/// Plug into `crate_versions()` to convert to machine-readable form.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::crate_versions_raw;
/// let raw_versions = crate_versions_raw("checksums");
/// ```
pub fn crate_versions_raw(crate_name: &str) -> String {
    let mut buf = String::new();
    HttpClient::new()
        .get(&format!("https://crates.io/api/v1/crates/{}/versions", crate_name))
        .send()
        .unwrap()
        .read_to_string(&mut buf)
        .unwrap();
    buf
}

/// Parse the raw crate version list into a collection of `Semver`s.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::{crate_versions_raw, crate_versions};
/// # let raw_versions = crate_versions_raw("checksums");
/// let versions = crate_versions(&raw_versions);
///
/// println!("Released versions of checksums:");
/// for ver in &versions {
///     println!("  {}", ver);
/// }
/// ```
pub fn crate_versions(raw: &str) -> Vec<Semver> {
    json::parse(raw).unwrap()["versions"].members().map(|v| Semver::parse(v["num"].as_str().unwrap()).unwrap()).collect()
}
