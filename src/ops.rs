//! Main functions doing actual work.
//!
//! Use `installed_main_repo_packages()` to list the installed packages,
//! then use `intersect_packages()` to confirm which ones should be updated,
//! poll the packages' latest versions by calling `MainRepoPackage::pull_version` on them,
//! continue with doing whatever you wish.


use std::path::{PathBuf, Path};
use semver::Version as Semver;
use std::fs::{self, File};
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
/// # use cargo_update::ops::{MainRepoPackage, get_index_path};
/// # use semver::Version as Semver;
/// # use std::fs::{self, File};
/// # use std::io::Write;
/// # use std::env;
/// # fn main() {
/// # let mut cargo_dir = env::temp_dir();
/// # cargo_dir.push("cargo_update-doctest");
/// # let _ = fs::create_dir(&cargo_dir);
/// # cargo_dir.push("MainRepoPackage-0");
/// # let _ = fs::create_dir(&cargo_dir);
/// # File::create(["registry", "index", "github.com-1ecc6299db9ec823", "ra", "ce"].into_iter()
/// #     .fold(cargo_dir.clone(), |pb, chunk| {
/// #         let _ = fs::create_dir(pb.join(chunk));
/// #         pb.join(chunk)
/// #     }).join("racer"))
/// # .unwrap().write_all(br#"{"vers": "1.2.10", "yanked": false}"#).unwrap();
/// let registry = get_index_path(&cargo_dir);
/// let package_s = "racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)";
/// let mut package = MainRepoPackage::parse(package_s).unwrap();
/// assert_eq!(package,
///            MainRepoPackage {
///                name: "racer".to_string(),
///                version: Semver::parse("1.2.10").unwrap(),
///                newest_version: None,
///            });
///
/// package.pull_version(&registry);
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
    /// # use cargo_update::ops::{MainRepoPackage, get_index_path};
    /// # use std::fs::{self, File};
    /// # use std::io::Write;
    /// # use std::env;
    /// # let mut cargo_dir = env::temp_dir();
    /// # cargo_dir.push("cargo_update-doctest");
    /// # let _ = fs::create_dir(&cargo_dir);
    /// # cargo_dir.push("MainRepoPackage-pull_version-0");
    /// # let _ = fs::create_dir(&cargo_dir);
    /// # File::create(["registry", "index", "github.com-1ecc6299db9ec823", "ra", "ce"].into_iter()
    /// #     .fold(cargo_dir.clone(), |pb, chunk| {
    /// #         let _ = fs::create_dir(pb.join(chunk));
    /// #         pb.join(chunk)
    /// #     }).join("racer"))
    /// # .unwrap().write_all(br#"{"vers": "1.2.10", "yanked": false}"#).unwrap();
    /// let registry = get_index_path(&cargo_dir);
    /// let package_s = "racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)";
    /// let mut package = MainRepoPackage::parse(package_s).unwrap();
    /// package.pull_version(&registry);
    /// assert!(package.newest_version.is_some());
    /// ```
    pub fn pull_version(&mut self, registry: &Path) {
        let vers = crate_versions(&find_package_data(&self.name, registry).unwrap());
        self.newest_version = vers.into_iter().max();
    }
}


/// [Follow `install.root`](https://github.com/nabijaczleweli/cargo-update/issues/23) in the `.crates.toml` file in the
/// specified directory up to the final one.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::resolve_cargo_directory;
/// # use std::env::temp_dir;
/// # let cargo_dir = temp_dir();
/// let cargo_dir = resolve_cargo_directory(cargo_dir);
/// # let _ = cargo_dir;
/// ```
pub fn resolve_cargo_directory(cargo_dir: PathBuf) -> PathBuf {
    let crates_path = cargo_dir.join(".crates.toml");
    if crates_path.exists() {
        let mut crates = String::new();
        File::open(crates_path).unwrap().read_to_string(&mut crates).unwrap();

        if let Some(idir) = toml::Parser::new(&crates)
            .parse()
            .unwrap()
            .get("install")
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("root"))
            .and_then(|t| t.as_str()) {
            return resolve_cargo_directory(PathBuf::from(idir));
        }
    }
    cargo_dir
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

/// Parse the raw crate descriptor from the repository into a collection of `Semver`s.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::crate_versions;
/// # use std::path::PathBuf;
/// # let desc_path = PathBuf::from("test-data/checksums-versions.json");
/// let versions = crate_versions(&desc_path);
///
/// println!("Released versions of checksums:");
/// for ver in &versions {
///     println!("  {}", ver);
/// }
/// ```
pub fn crate_versions(package_desc: &Path) -> Vec<Semver> {
    let mut buf = String::new();
    File::open(package_desc).unwrap().read_to_string(&mut buf).unwrap();

    buf.lines()
        .map(|p| json::parse(p).unwrap())
        .filter(|j| !j["yanked"].as_bool().unwrap())
        .map(|j| Semver::parse(j["vers"].as_str().unwrap()).unwrap())
        .collect()
}

/// Get the location of the latest registry index in the specified cargo directory.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::get_index_path;
/// # use std::env::temp_dir;
/// # use std::fs;
/// # let mut cargo_dir = temp_dir();
/// # let _ = fs::create_dir(&cargo_dir);
/// # cargo_dir.push("cargo_update-doctest");
/// # let _ = fs::create_dir(&cargo_dir);
/// # cargo_dir.push("get_index_path-0");
/// # let _ = fs::create_dir(&cargo_dir);
/// # let idx_dir = cargo_dir.join("registry").join("index").join("github.com-1ecc6299db9ec823");
/// # let _ = fs::create_dir_all(&idx_dir);
/// let index = get_index_path(&cargo_dir);
/// // Use find_package_data() to look for packages
/// # assert_eq!(index, idx_dir);
/// ```
pub fn get_index_path(cargo_dir: &Path) -> PathBuf {
    fs::read_dir(cargo_dir.join("registry").join("index"))
        .unwrap()
        .map(Result::unwrap)
        .filter(|i| i.file_type().unwrap().is_dir())
        .max_by_key(|i| i.metadata().unwrap().modified().unwrap())
        .unwrap()
        .path()
}

/// Find a package in the cargo index.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::find_package_data;
/// # use std::fs::{self, File};
/// # use std::env::temp_dir;
/// # let mut index_dir = temp_dir();
/// # let _ = fs::create_dir(&index_dir);
/// # index_dir.push("cargo_update-doctest");
/// # let _ = fs::create_dir(&index_dir);
/// # index_dir.push("find_package_data-0");
/// # let _ = fs::create_dir(&index_dir);
/// # let _ = fs::create_dir_all(index_dir.join("ca").join("rg"));
/// # File::create(index_dir.join("ca").join("rg").join("cargo")).unwrap();
/// # let cargo =
/// find_package_data("cargo", &index_dir);
/// # assert_eq!(cargo, Some(index_dir.join("ca").join("rg").join("cargo")));
/// ```
pub fn find_package_data(cratename: &str, index_dir: &Path) -> Option<PathBuf> {
    let maybepath = |pb: PathBuf| if pb.exists() { Some(pb) } else { None };

    match cratename.len() {
        0 => panic!("0-length cratename"),
        1 | 2 => maybepath(index_dir.join(cratename.len().to_string())).and_then(|pb| maybepath(pb.join(cratename))),
        3 => {
            maybepath(index_dir.join("3"))
                .and_then(|pb| maybepath(pb.join(&cratename[0..1])))
                .and_then(|pb| maybepath(pb.join(cratename)))
        }
        _ => {
            maybepath(index_dir.join(&cratename[0..2]))
                .and_then(|pb| maybepath(pb.join(&cratename[2..4])))
                .and_then(|pb| maybepath(pb.join(cratename)))
        }
    }
}
