//! Main functions doing actual work.
//!
//! Use `installed_main_repo_packages()` to list the installed packages,
//! then use `intersect_packages()` to confirm which ones should be updated,
//! poll the packages' latest versions by calling `MainRepoPackage::pull_version` on them,
//! continue with doing whatever you wish.


use git2::{self, Error as GitError, Repository, Tree, Oid};
use semver::{VersionReq as SemverReq, Version as Semver};
use std::fs::{self, DirEntry, File};
use std::path::{PathBuf, Path};
use std::time::SystemTime;
use std::io::Read;
use regex::Regex;
use url::Url;
use std::cmp;
use toml;
use json;

mod config;

pub use self::config::*;


lazy_static! {
    static ref MAIN_PACKAGE_RGX: Regex = Regex::new(r"([^\s]+) ([^\s]+) \(registry+\+([^\s]+)\)").unwrap();
    static ref GIT_PACKAGE_RGX: Regex = Regex::new(r"([^\s]+) ([^\s]+) \(git+\+([^#\s]+)#([^\s]{40})\)").unwrap();
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
///                max_version: None,
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
    /// User-bounded maximum version to update up to.
    pub max_version: Option<Semver>,
}

/// A representation of a package a remote git repository.
///
/// The newest commit is pulled from that repo via `pull_version()`.
///
/// The `parse()` function parses the format used in `$HOME/.cargo/.crates.toml`.
///
/// # Examples
///
/// ```
/// # extern crate cargo_update;
/// # extern crate git2;
/// # use cargo_update::ops::GitRepoPackage;
/// # fn main() {
/// let package_s = "alacritty 0.1.0 (git+https://github.com/jwilm/alacritty#eb231b3e70b87875df4bdd1974d5e94704024d70)";
/// let mut package = GitRepoPackage::parse(package_s).unwrap();
/// assert_eq!(package,
///            GitRepoPackage {
///                name: "alacritty".to_string(),
///                url: "https://github.com/jwilm/alacritty".to_string(),
///                branch: None,
///                id: git2::Oid::from_str("eb231b3e70b87875df4bdd1974d5e94704024d70").unwrap(),
///                newest_id: None,
///            });
///
/// # /*
/// package.pull_version(&registry_tree, &registry);
/// # */
/// # package.newest_id = Some(git2::Oid::from_str("5f7885749c4d7e48869b1fc0be4d430601cdbbfa").unwrap());
/// assert!(package.newest_id.is_some());
/// # }
/// ```
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct GitRepoPackage {
    /// The package's name.
    pub name: String,
    /// The remote git repo URL.
    pub url: String,
    /// The installed branch, or `None` for default.
    pub branch: Option<String>,
    /// The package's locally installed version's object hash.
    pub id: Oid,
    /// The latest version of the package vailable at the main [`crates.io`](https://crates.io) repository.
    ///
    /// `None` by default, acquire via `MainRepoPackage::pull_version()`.
    pub newest_id: Option<Oid>,
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
    ///                max_version: None,
    ///            });
    ///
    /// let package_s = "cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)";
    /// assert_eq!(MainRepoPackage::parse(package_s).unwrap(),
    ///            MainRepoPackage {
    ///                name: "cargo-outdated".to_string(),
    ///                version: Some(Semver::parse("0.2.0").unwrap()),
    ///                newest_version: None,
    ///                max_version: None,
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
        MAIN_PACKAGE_RGX.captures(what).map(|c| {
            MainRepoPackage {
                name: c.get(1).unwrap().as_str().to_string(),
                version: Some(Semver::parse(c.get(2).unwrap().as_str()).unwrap()),
                newest_version: None,
                max_version: None,
            }
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
    /// # use semver::{VersionReq as SemverReq, Version as Semver};
    /// # use cargo_update::ops::MainRepoPackage;
    /// # use std::str::FromStr;
    /// # fn main() {
    /// assert!(MainRepoPackage {
    ///             name: "racer".to_string(),
    ///             version: Some(Semver::parse("1.7.2").unwrap()),
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///             max_version: None,
    ///         }.needs_update(None));
    /// assert!(MainRepoPackage {
    ///             name: "racer".to_string(),
    ///             version: None,
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///             max_version: None,
    ///         }.needs_update(None));
    /// assert!(!MainRepoPackage {
    ///             name: "racer".to_string(),
    ///             version: Some(Semver::parse("2.0.6").unwrap()),
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///             max_version: None,
    ///         }.needs_update(None));
    /// assert!(!MainRepoPackage {
    ///             name: "racer".to_string(),
    ///             version: Some(Semver::parse("2.0.6").unwrap()),
    ///             newest_version: None,
    ///             max_version: None,
    ///         }.needs_update(None));
    ///
    /// let req = SemverReq::from_str("^1.7").unwrap();
    /// assert!(MainRepoPackage {
    ///             name: "racer".to_string(),
    ///             version: Some(Semver::parse("1.7.2").unwrap()),
    ///             newest_version: Some(Semver::parse("1.7.3").unwrap()),
    ///             max_version: None,
    ///         }.needs_update(Some(&req)));
    /// assert!(MainRepoPackage {
    ///             name: "racer".to_string(),
    ///             version: None,
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///             max_version: None,
    ///         }.needs_update(Some(&req)));
    /// assert!(!MainRepoPackage {
    ///             name: "racer".to_string(),
    ///             version: Some(Semver::parse("1.7.2").unwrap()),
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///             max_version: None,
    ///         }.needs_update(Some(&req)));
    /// # }
    /// ```
    pub fn needs_update(&self, req: Option<&SemverReq>) -> bool {
        (req.into_iter().zip(self.version.as_ref()).map(|(sr, cv)| !sr.matches(cv)).next().unwrap_or(true) ||
         req.into_iter().zip(self.update_to_version()).map(|(sr, uv)| sr.matches(uv)).next().unwrap_or(true)) &&
        self.update_to_version().map(|upd_v| self.version.is_none() || (*self.version.as_ref().unwrap() < *upd_v)).unwrap_or(false)
    }

    /// Get package version to update to, or `None` if the crate has no newest version (was yanked)
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cargo_update;
    /// # extern crate semver;
    /// # use cargo_update::ops::MainRepoPackage;
    /// # use semver::Version as Semver;
    /// # fn main() {
    /// assert_eq!(MainRepoPackage {
    ///                name: "racer".to_string(),
    ///                version: Some(Semver::parse("1.7.2").unwrap()),
    ///                newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///                max_version: Some(Semver::parse("2.0.5").unwrap()),
    ///            }.update_to_version(),
    ///            Some(&Semver::parse("2.0.5").unwrap()));
    /// assert_eq!(MainRepoPackage {
    ///                name: "gutenberg".to_string(),
    ///                version: Some(Semver::parse("0.0.7").unwrap()),
    ///                newest_version: None,
    ///                max_version: None,
    ///            }.update_to_version(),
    ///            None);
    /// # }
    /// ```
    pub fn update_to_version(&self) -> Option<&Semver> {
        self.newest_version.as_ref().map(|new_v| cmp::min(new_v, self.max_version.as_ref().unwrap_or(new_v)))
    }
}

impl GitRepoPackage {
    /// Try to decypher a package descriptor into a `GitRepoPackage`.
    ///
    /// Will return `None` if:
    ///
    ///   * the given package descriptor is invalid, or
    ///   * the package descriptor is not from a .
    ///
    /// In the returned instance, `newest_version` is always `None`, get it via `GitRepoPackage::pull_version()`.
    ///
    /// # Examples
    ///
    /// Remote git repo packages:
    ///
    /// ```
    /// # extern crate cargo_update;
    /// # extern crate git2;
    /// # use cargo_update::ops::GitRepoPackage;
    /// # fn main() {
    /// let package_s = "alacritty 0.1.0 (git+https://github.com/jwilm/alacritty#eb231b3e70b87875df4bdd1974d5e94704024d70)";
    /// assert_eq!(GitRepoPackage::parse(package_s).unwrap(),
    ///            GitRepoPackage {
    ///                name: "alacritty".to_string(),
    ///                url: "https://github.com/jwilm/alacritty".to_string(),
    ///                branch: None,
    ///                id: git2::Oid::from_str("eb231b3e70b87875df4bdd1974d5e94704024d70").unwrap(),
    ///                newest_id: None,
    ///            });
    ///
    /// let package_s = "chattium-oxide-client 0.1.0 \
    ///                  (git+https://github.com/nabijaczleweli/chattium-oxide-client\
    ///                       ?branch=master#108a7b94f0e0dcb2a875f70fc0459d5a682df14c)";
    /// assert_eq!(GitRepoPackage::parse(package_s).unwrap(),
    ///            GitRepoPackage {
    ///                name: "chattium-oxide-client".to_string(),
    ///                url: "https://github.com/nabijaczleweli/chattium-oxide-client".to_string(),
    ///                branch: Some("master".to_string()),
    ///                id: git2::Oid::from_str("108a7b94f0e0dcb2a875f70fc0459d5a682df14c").unwrap(),
    ///                newest_id: None,
    ///            });
    /// # }
    /// ```
    ///
    /// Main repository package:
    ///
    /// ```
    /// # use cargo_update::ops::GitRepoPackage;
    /// let package_s = "racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)";
    /// assert!(GitRepoPackage::parse(package_s).is_none());
    /// ```
    pub fn parse(what: &str) -> Option<GitRepoPackage> {
        GIT_PACKAGE_RGX.captures(what).map(|c| {
            let mut url = Url::parse(c.get(3).unwrap().as_str()).unwrap();
            let branch = url.query_pairs().find(|&(ref name, _)| name == "branch").map(|(_, value)| value.to_string());
            url.set_query(None);
            GitRepoPackage {
                name: c.get(1).unwrap().as_str().to_string(),
                url: url.into_string(),
                branch: branch,
                id: Oid::from_str(c.get(4).unwrap().as_str()).unwrap(),
                newest_id: None,
            }
        })
    }

    /// Clone the repo and check what the latest commit's hash is.
    pub fn pull_version<P: AsRef<Path>>(&mut self, temp_dir: P) {
        fs::create_dir_all(temp_dir.as_ref()).unwrap();
        let clone_dir = temp_dir.as_ref().join(&self.name);
        let repo = if clone_dir.exists() {
            let mut r = git2::Repository::open(clone_dir);
            if let Ok(ref mut r) = r.as_mut() {
                r.find_remote("origin").and_then(|mut rm| rm.fetch(&[self.branch.as_ref().map(String::as_str).unwrap_or("master")], None, None)).unwrap();
                r.set_head("FETCH_HEAD").unwrap();
            }
            r
        } else {
            let mut bldr = git2::build::RepoBuilder::new();
            bldr.bare(true);
            if let Some(ref b) = self.branch.as_ref() {
                bldr.branch(b);
            }
            bldr.clone(&self.url, &clone_dir)
        };

        self.newest_id = Some(repo.and_then(|r| r.head().and_then(|h| h.target().ok_or_else(|| GitError::from_str("HEAD not a direct reference")))).unwrap());
    }

    /// Check whether this package needs to be installed
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cargo_update;
    /// # extern crate git2;
    /// # use cargo_update::ops::GitRepoPackage;
    /// # fn main() {
    /// assert!(GitRepoPackage {
    ///             name: "alacritty".to_string(),
    ///             url: "https://github.com/jwilm/alacritty".to_string(),
    ///             branch: None,
    ///             id: git2::Oid::from_str("eb231b3e70b87875df4bdd1974d5e94704024d70").unwrap(),
    ///             newest_id: Some(git2::Oid::from_str("5f7885749c4d7e48869b1fc0be4d430601cdbbfa").unwrap()),
    ///         }.needs_update());
    /// assert!(!GitRepoPackage {
    ///             name: "alacritty".to_string(),
    ///             url: "https://github.com/jwilm/alacritty".to_string(),
    ///             branch: None,
    ///             id: git2::Oid::from_str("5f7885749c4d7e48869b1fc0be4d430601cdbbfa").unwrap(),
    ///             newest_id: Some(git2::Oid::from_str("5f7885749c4d7e48869b1fc0be4d430601cdbbfa").unwrap()),
    ///         }.needs_update());
    /// # }
    /// ```
    pub fn needs_update(&self) -> bool {
        self.newest_id.is_some() && self.id != *self.newest_id.as_ref().unwrap()
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
/// This also deduplicates packages and assumes the latest version as the correct one to work around
/// [#44](https://github.com/nabijaczleweli/cargo-update/issues/44) a.k.a.
/// [rust-lang/cargo#4321](https://github.com/rust-lang/cargo/issues/4321).
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

        let mut res = Vec::<MainRepoPackage>::new();
        for pkg in toml::from_str::<toml::Value>(&crates).unwrap()["v1"].as_table().unwrap().keys().flat_map(|s| MainRepoPackage::parse(s)) {
            if let Some(saved) = res.iter_mut().find(|p| p.name == pkg.name) {
                if saved.version.is_none() || saved.version.as_ref().unwrap() < pkg.version.as_ref().unwrap() {
                    saved.version = pkg.version;
                }
                continue;
            }

            res.push(pkg);
        }
        res
    } else {
        Vec::new()
    }
}

/// List the installed packages at the specified location that originate
/// from a  remote git repository.
///
/// If the `.crates.toml` file doesn't exist an empty vector is returned.
///
/// This also deduplicates packages and assumes the latest-mentioned version as the most correct.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::installed_git_repo_packages;
/// # use std::env::temp_dir;
/// # let cargo_dir = temp_dir().join(".crates.toml");
/// let packages = installed_git_repo_packages(&cargo_dir);
/// for package in &packages {
///     println!("{} v{}", package.name, package.id);
/// }
/// ```
pub fn installed_git_repo_packages(crates_file: &Path) -> Vec<GitRepoPackage> {
    if crates_file.exists() {
        let mut crates = String::new();
        File::open(crates_file).unwrap().read_to_string(&mut crates).unwrap();

        let mut res = Vec::<GitRepoPackage>::new();
        for pkg in toml::from_str::<toml::Value>(&crates).unwrap()["v1"].as_table().unwrap().keys().flat_map(|s| GitRepoPackage::parse(s)) {
            if let Some(saved) = res.iter_mut().find(|p| p.name == pkg.name) {
                saved.id = pkg.id;
                continue;
            }

            res.push(pkg);
        }
        res
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
/// # let packages_to_update = [("racer".to_string(), None), ("cargo-outdated".to_string(), None)];
/// let mut installed_packages = installed_main_repo_packages(&cargo_dir);
/// # let mut installed_packages =
/// #     vec![MainRepoPackage::parse("cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
/// #          MainRepoPackage::parse("racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
/// #          MainRepoPackage::parse("rustfmt 0.6.2 (registry+https://github.com/rust-lang/crates.io-index)").unwrap()];
/// installed_packages = intersect_packages(&installed_packages, &packages_to_update, false);
/// # assert_eq!(&installed_packages,
/// #   &[MainRepoPackage::parse("cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)").unwrap(),
/// #     MainRepoPackage::parse("racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)").unwrap()]);
/// ```
pub fn intersect_packages(installed: &[MainRepoPackage], to_update: &[(String, Option<Semver>)], allow_installs: bool) -> Vec<MainRepoPackage> {
    installed.iter()
        .filter(|p| to_update.iter().any(|u| p.name == u.0))
        .cloned()
        .map(|p| MainRepoPackage { max_version: to_update.iter().find(|u| p.name == u.0).and_then(|u| u.1.clone()), ..p })
        .chain(to_update.iter().filter(|p| allow_installs && installed.iter().find(|i| i.name == p.0).is_none()).map(|p| {
            MainRepoPackage {
                name: p.0.clone(),
                version: None,
                newest_version: None,
                max_version: p.1.clone(),
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
    let mut buf = String::new();
    package_desc.read_to_string(&mut buf).unwrap();

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
        .max_by_key(latest_modified)
        .unwrap()
        .path()
}

fn latest_modified(ent: &DirEntry) -> SystemTime {
    let meta = ent.metadata().unwrap();
    let mut latest = meta.modified().unwrap();
    if meta.is_dir() {
        for ent in fs::read_dir(ent.path()).unwrap() {
            latest = cmp::max(latest, latest_modified(&ent.unwrap()));
        }
    }
    latest
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
