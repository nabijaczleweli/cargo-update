//! Main functions doing actual work.
//!
//! Use `installed_main_repo_packages()` to list the installed packages,
//! then use `intersect_packages()` to confirm which ones should be updated,
//! poll the packages' latest versions by calling `MainRepoPackage::pull_version` on them,
//! continue with doing whatever you wish.


use std::path::{PathBuf, Path};
use semver::Version as Semver;
use git2::{Repository, Tree};
use std::fs::{self, File};
use std::io::Read;
use regex::Regex;
use toml;
use serde_json;

mod config;

pub use self::config::*;


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
///                version: Some(Semver::parse("1.2.10").unwrap()),
///                newest_version: None,
///            });
///
/// # /*
/// package.pull_version(&registry_tree, &registry);
/// # */
/// # package.newest_version = Some(Semver::parse("1.2.11").unwrap());
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
    pub version: Option<Semver>,
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
    ///                version: Some(Semver::parse("1.2.10").unwrap()),
    ///                newest_version: None,
    ///            });
    ///
    /// let package_s = "cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)";
    /// assert_eq!(MainRepoPackage::parse(package_s).unwrap(),
    ///            MainRepoPackage {
    ///                name: "cargo-outdated".to_string(),
    ///                version: Some(Semver::parse("0.2.0").unwrap()),
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
        PACKAGE_RGX.captures(what).and_then(|c| if c.get(3).unwrap().as_str() == "registry" {
            Some(MainRepoPackage {
                name: c.get(1).unwrap().as_str().to_string(),
                version: Some(Semver::parse(c.get(2).unwrap().as_str()).unwrap()),
                newest_version: None,
            })
        } else {
            None
        })
    }

    /// Download the version list for this crate off the specified repository tree.
    pub fn pull_version<'t>(&mut self, registry: &Tree<'t>, registry_parent: &'t Repository) {
        let vers = crate_versions(&mut &find_package_data(&self.name, registry, registry_parent).unwrap()[..]);
        self.newest_version = vers.into_iter().max();
    }

    /// Check whether this package needs to be installed
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cargo_update;
    /// # extern crate semver;
    /// # use cargo_update::ops::MainRepoPackage;
    /// # use semver::Version as Semver;
    /// # fn main() {
    /// assert!(MainRepoPackage {
    ///             name: "racer".to_string(),
    ///             version: Some(Semver::parse("1.7.2").unwrap()),
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///         }.needs_update());
    /// assert!(MainRepoPackage {
    ///             name: "racer".to_string(),
    ///             version: None,
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///         }.needs_update());
    /// assert!(!MainRepoPackage {
    ///             name: "racer".to_string(),
    ///             version: Some(Semver::parse("2.0.6").unwrap()),
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///         }.needs_update());
    /// # }
    /// ```
    pub fn needs_update(&self) -> bool {
        self.version.is_none() || *self.version.as_ref().unwrap() < *self.newest_version.as_ref().unwrap()
    }
}


/// [Follow `install.root`](https://github.com/nabijaczleweli/cargo-update/issues/23) in the `config` file
/// parallel to the specified crates file up to the final one.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::resolve_crates_file;
/// # use std::env::temp_dir;
/// # let crates_file = temp_dir().join(".crates.toml");
/// let crates_file = resolve_crates_file(crates_file);
/// # let _ = crates_file;
/// ```
pub fn resolve_crates_file(crates_file: PathBuf) -> PathBuf {
    let config_file = crates_file.with_file_name("config");
    if config_file.exists() {
        let mut crates = String::new();
        File::open(&config_file).unwrap().read_to_string(&mut crates).unwrap();

        if let Some(idir) = toml::from_str::<toml::Value>(&crates)
            .unwrap()
            .get("install")
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("root"))
            .and_then(|t| t.as_str()) {
            return resolve_crates_file(Path::new(idir).join(".crates.toml"));
        }
    }
    crates_file
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
/// # let cargo_dir = temp_dir().join(".crates.toml");
/// let packages = installed_main_repo_packages(&cargo_dir);
/// for package in &packages {
///     println!("{} v{}", package.name, package.version.as_ref().unwrap());
/// }
/// ```
pub fn installed_main_repo_packages(crates_file: &Path) -> Vec<MainRepoPackage> {
    if crates_file.exists() {
        let mut crates = String::new();
        File::open(crates_file).unwrap().read_to_string(&mut crates).unwrap();

        toml::from_str::<toml::Value>(&crates).unwrap()["v1"].as_table().unwrap().keys().flat_map(|s| MainRepoPackage::parse(s)).collect()
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
/// installed_packages = intersect_packages(installed_packages, &packages_to_update, false);
/// # assert_eq!(&installed_packages,
/// #   &[MainRepoPackage::parse("cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
/// #     MainRepoPackage::parse("racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)").unwrap()]);
/// ```
pub fn intersect_packages(installed: Vec<MainRepoPackage>, to_update: &[String], allow_installs: bool) -> Vec<MainRepoPackage> {
    installed.iter()
        .filter(|p| to_update.contains(&p.name))
        .cloned()
        .chain(to_update.iter().filter(|p| allow_installs && installed.iter().find(|i| i.name == **p).is_none()).map(|p| {
            MainRepoPackage {
                name: p.clone(),
                version: None,
                newest_version: None,
            }
        }))
        .collect()
}

/// Parse the raw crate descriptor from the repository into a collection of `Semver`s.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::crate_versions;
/// # use std::fs::File;
/// # let desc_path = "test-data/checksums-versions.json";
/// let versions = crate_versions(&mut File::open(desc_path).unwrap());
///
/// println!("Released versions of checksums:");
/// for ver in &versions {
///     println!("  {}", ver);
/// }
/// ```
pub fn crate_versions<R: Read>(package_desc: &mut R) -> Vec<Semver> {
    #[derive(Deserialize)]
    struct PackageDesc {
        yanked: bool,
        vers: Semver,
    }

    serde_json::Deserializer::from_reader(package_desc)
        .into_iter::<PackageDesc>()
        .map(Result::unwrap)
        .filter(|pkg| !pkg.yanked)
        .map(|pkg| pkg.vers)
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

/// Find package data in the specified cargo index tree.
pub fn find_package_data<'t>(cratename: &str, registry: &Tree<'t>, registry_parent: &'t Repository) -> Option<Vec<u8>> {
    macro_rules! try_opt {
        ($expr:expr) => {
            match $expr {
                Some(e) => e,
                None => return None,
            }
        }
    }

    let clen = cratename.len().to_string();
    let mut elems = Vec::new();
    if cratename.len() <= 3 {
        elems.push(&clen[..]);
    }
    match cratename.len() {
        0 => panic!("0-length cratename"),
        1 | 2 => {}
        3 => elems.push(&cratename[0..1]),
        _ => {
            elems.push(&cratename[0..2]);
            elems.push(&cratename[2..4]);
        }
    }
    elems.push(cratename);

    let ent = try_opt!(registry.get_name(elems[0]));
    let obj = try_opt!(ent.to_object(registry_parent).ok());
    let ent = try_opt!(try_opt!(obj.as_tree()).get_name(elems[1]));
    let obj = try_opt!(ent.to_object(registry_parent).ok());
    if elems.len() == 3 {
        let ent = try_opt!(try_opt!(obj.as_tree()).get_name(elems[2]));
        let obj = try_opt!(ent.to_object(registry_parent).ok());
        Some(try_opt!(obj.as_blob()).content().into())
    } else {
        Some(try_opt!(obj.as_blob()).content().into())
    }
}
