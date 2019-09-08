//! Main functions doing actual work.
//!
//! Use `installed_main_repo_packages()` to list the installed packages,
//! then use `intersect_packages()` to confirm which ones should be updated,
//! poll the packages' latest versions by calling `MainRepoPackage::pull_version()` on them,
//! continue with doing whatever you wish.


use git2::{self, Error as GitError, Config as GitConfig, Cred as GitCred, RemoteCallbacks, CredentialType, FetchOptions, ProxyOptions, Repository, Tree, Oid};
use semver::{VersionReq as SemverReq, Version as Semver};
use std::fs::{self, DirEntry, File};
use std::path::{PathBuf, Path};
use std::io::{Write, Read};
use std::time::SystemTime;
use std::borrow::Cow;
use std::{cmp, env};
use regex::Regex;
use url::Url;
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
        let vers = crate_versions(&mut &find_package_data(&self.name, registry, registry_parent)
            .ok_or_else(|| format!("package {} not found", self.name))
            .unwrap()
                                            [..]);
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
    pub fn pull_version<Pt: AsRef<Path>, Pg: AsRef<Path>>(&mut self, temp_dir: Pt, git_db_dir: Pg, http_proxy: Option<&str>) {
        self.pull_version_impl(temp_dir.as_ref(), git_db_dir.as_ref(), http_proxy)
    }

    fn pull_version_impl(&mut self, temp_dir: &Path, git_db_dir: &Path, http_proxy: Option<&str>) {
        let clone_dir = find_git_db_repo(git_db_dir, &self.name).unwrap_or_else(|| {
            fs::create_dir_all(temp_dir).unwrap();
            temp_dir.join(&self.name)
        });

        let repo = if clone_dir.exists() {
            let mut r = Repository::open(clone_dir);
            if let Ok(ref mut r) = r.as_mut() {
                r.find_remote("origin")
                    .or_else(|_| r.remote_anonymous(&self.url))
                    .and_then(|mut rm| {
                        with_authentication(&self.url, |creds| {
                            let mut cb = RemoteCallbacks::new();
                            cb.credentials(|a, b, c| creds(a, b, c));

                            rm.fetch(&[self.branch.as_ref().map(String::as_str).unwrap_or("master")],
                                     Some(&mut fetch_options_from_proxy_url_and_callbacks(http_proxy, cb)),
                                     None)
                        })
                    })
                    .unwrap();
                r.set_head("FETCH_HEAD").unwrap();
            }
            r
        } else {
            with_authentication(&self.url, |creds| {
                let mut bldr = git2::build::RepoBuilder::new();

                let mut cb = RemoteCallbacks::new();
                cb.credentials(|a, b, c| creds(a, b, c));
                bldr.fetch_options(fetch_options_from_proxy_url_and_callbacks(http_proxy, cb));
                if let Some(ref b) = self.branch.as_ref() {
                    bldr.branch(b);
                }

                bldr.bare(true);
                bldr.clone(&self.url, &clone_dir)
            })
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


/// One of elements with which to filter required packages.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageFilterElement {
    /// Requires toolchain to be specified to the specified toolchain.
    ///
    /// Parsed name: `"toolchain"`.
    Toolchain(String),
}

impl PackageFilterElement {
    /// Parse one filter specifier into up to one package filter
    ///
    /// # Examples
    ///
    /// ```
    /// # use cargo_update::ops::PackageFilterElement;
    /// assert_eq!(PackageFilterElement::parse("toolchain=nightly"),
    ///            Ok(PackageFilterElement::Toolchain("nightly".to_string())));
    ///
    /// assert!(PackageFilterElement::parse("capitalism").is_err());
    /// assert!(PackageFilterElement::parse("communism=good").is_err());
    /// ```
    pub fn parse(from: &str) -> Result<PackageFilterElement, String> {
        let (key, value) = from.split_at(from.find('=').ok_or_else(|| format!(r#"Filter string "{}" does not contain the key/value separator "=""#, from))?);
        let value = &value[1..];

        Ok(match key {
            "toolchain" => PackageFilterElement::Toolchain(value.to_string()),
            _ => return Err(format!(r#"Unrecognised filter key "{}""#, key)),
        })
    }

    /// Check if the specified package config matches this filter element.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cargo_update::ops::{PackageFilterElement, ConfigOperation, PackageConfig};
    /// assert!(PackageFilterElement::Toolchain("nightly".to_string())
    ///     .matches(&PackageConfig::from(&[ConfigOperation::SetToolchain("nightly".to_string())])));
    ///
    /// assert!(!PackageFilterElement::Toolchain("nightly".to_string()).matches(&PackageConfig::from(&[])));
    /// ```
    pub fn matches(&self, cfg: &PackageConfig) -> bool {
        match *self {
            PackageFilterElement::Toolchain(ref chain) => Some(chain) == cfg.toolchain.as_ref(),
        }
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
    crate_versions_impl(buf)
}

fn crate_versions_impl(buf: String) -> Vec<Semver> {
    buf.lines()
        .map(|p| json::parse(p).unwrap())
        .filter(|j| !j["yanked"].as_bool().unwrap())
        .map(|j| Semver::parse(j["vers"].as_str().unwrap()).unwrap())
        .collect()
}

/// Get the location of the latest registry index whose name optionally starts with the registry URL's domain name in the
/// specified cargo directory.
///
/// If no indices exist, an appropriate `Err` is returned.
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
/// // These are equivalent for most users,
/// // but you might need to specify the URL to find the right checkout if you use more than one registry
/// let index = get_index_path(&cargo_dir, None).unwrap();
/// let index = get_index_path(&cargo_dir, Some("https://github.com/rust-lang/crates.io-index")).unwrap();
///
/// // Use find_package_data() to look for packages
/// # assert_eq!(index, idx_dir);
/// ```
pub fn get_index_path(cargo_dir: &Path, registry_url: Option<&str>) -> Result<PathBuf, &'static str> {
    let registry_url = registry_url.map(|u| Url::parse(u).map_err(|_| "registry URL not an URL")).transpose()?;
    let registry_host = registry_url.as_ref().and_then(|u| u.host_str());

    Ok(fs::read_dir(cargo_dir.join("registry").join("index"))
        .map_err(|_| "index directory nonexistant")?
        .map(Result::unwrap)
        .filter(|i| i.file_type().unwrap().is_dir())
        .filter(|i| registry_host.map(|rh| i.file_name().to_string_lossy().starts_with(rh)).unwrap_or(true))
        .max_by_key(latest_modified)
        .ok_or("empty index directory")?
        .path())
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

/// Update the specified index repository from the specified URL.
///
/// Historically, `cargo search` was used, first of an
/// [empty string](https://github.com/nabijaczleweli/cargo-update/commit/aa090b4a38a486654cd73b173c3f49f6a56aa059#diff-639fbc4ef05b315af92b4d836c31b023R24),
/// then a [ZWNJ](https://github.com/nabijaczleweli/cargo-update/commit/aeccbd6252a2ddc90dc796117cefe327fbd7fb58#diff-639fbc4ef05b315af92b4d836c31b023R48)
/// ([why?](https://github.com/nabijaczleweli/cargo-update/commit/08a7111831c6397b7d67a51f9b77bee0a3bbbed4#diff-639fbc4ef05b315af92b4d836c31b023R47)).
///
/// The need for this in-house has first emerged with [#93](https://github.com/nabijaczleweli/cargo-update/issues/93): since
/// [`cargo` v1.29.0-nightly](https://github.com/rust-lang/cargo/pull/5621/commits/5e680f2849e44ce9dfe44416c3284a3b30747e74),
/// the registry was no longer updated.
/// So a [two-year-old `cargo` issue](https://github.com/rust-lang/cargo/issues/3377#issuecomment-417950125) was dug up,
/// asking for a `cargo update-registry` command, followed by a [PR](https://github.com/rust-lang/cargo/pull/5961) implementing
/// this.
/// Up to this point, there was no good substitute: `cargo install lazy_static`, the poster-child of replacements errored out
/// and left garbage in the console,
/// making it unsuitable.
///
/// But then, a [man of steel eyes and hawk will](https://github.com/Eh2406) has emerged, seemingly from nowhere, remarking:
///
/// > [21:09] Eh2406:
/// https://github.com/rust-lang/cargo/blob/1ee1ef0ea7ab47d657ca675e3b1bd2fcd68b5aab/src/cargo/sources/registry/remote.
/// rs#L204<br />
/// > [21:10] Eh2406: looks like it is a git fetch of "refs/heads/master:refs/remotes/origin/master"<br />
/// > [21:11] Eh2406: You are already poking about in cargos internal representation of the index, is this so much more?
///
/// It, well, isn't. And with some `cargo` maintainers being firmly against blind-merging that `cargo update-registry` PR,
/// here I go recycling <del>the same old song</del> that implementation (but simpler, and badlier).
///
/// Honourable mentions:
/// * [**@joshtriplett**](https://github.com/joshtriplett), for being a bastion for the people and standing with me in
///   advocacy for `cargo update-registry`
///   (NB: it was *his* issue from 2016 requesting it, funny how things turn around)
/// * [**@alexcrichton**](https://github.com/alexcrichton), for not getting overly too fed up with me while managing that PR
///   and producing a brilliant
///   argument list for doing it in-house (as well as suggesting I write another crate for this)
/// * And lastly, because mostly, [**@Eh2406**](https://github.com/Eh2406), for swooping in and saving me in my hour of
///   <del>need</del> not having a good replacement.
///
/// Most of this would have been impossible, of course, without the [`rust-lang` Discord server](https://discord.gg/rust-lang),
/// so shoutout to whoever convinced people that Discord is actually good.
pub fn update_index<W: Write>(index_repo: &mut Repository, repo_url: &str, http_proxy: Option<&str>, out: &mut W) -> Result<(), String> {
    writeln!(out, "    Updating registry '{}'", repo_url).map_err(|_| "failed to write updating message".to_string())?;
    index_repo.remote_anonymous(repo_url)
        .and_then(|mut r| {
            with_authentication(repo_url, |creds| {
                let mut cb = RemoteCallbacks::new();
                cb.credentials(|a, b, c| creds(a, b, c));

                r.fetch(&["refs/heads/master:refs/remotes/origin/master"],
                        Some(&mut fetch_options_from_proxy_url_and_callbacks(http_proxy, cb)),
                        None)
            })
        })
        .map_err(|e| e.message().to_string())?;
    writeln!(out).map_err(|_| "failed to write post-update newline".to_string())?;

    Ok(())
}

fn fetch_options_from_proxy_url_and_callbacks<'a>(proxy_url: Option<&str>, callbacks: RemoteCallbacks<'a>) -> FetchOptions<'a> {
    let mut ret = FetchOptions::new();
    if let Some(proxy_url) = proxy_url {
        ret.proxy_options({
            let mut prx = ProxyOptions::new();
            prx.url(proxy_url);
            prx
        });
    }
    ret.remote_callbacks(callbacks);
    ret
}

/// Get the URL to update index from the config file parallel to the specified crates file
///
/// Get `source.crates-io.registry` or follow `source.crates-io.replace-with`,
/// to `source.$SRCNAME.registry` or follow `source.$SRCNAME.replace-with`,
/// et caetera.
///
/// Defaults to `https://github.com/rust-lang/crates.io-index`
///
/// Consult [#107](https://github.com/nabijaczleweli/cargo-update/issues/107) for details
pub fn get_index_url(crates_file: &Path) -> Cow<'static, str> {
    get_index_url_impl(crates_file).map(Cow::from).unwrap_or(Cow::from("https://github.com/rust-lang/crates.io-index"))
}

fn get_index_url_impl(crates_file: &Path) -> Option<String> {
    let config = fs::read_to_string(crates_file.with_file_name("config")).ok()?;
    let config = toml::from_str::<toml::Value>(&config).ok()?;

    let sources = config.get("source")?;

    let mut cur_source = sources.get("crates-io")?;
    loop {
        match cur_source.get("replace-with") {
            Some(redir) => cur_source = sources.get(redir.as_str()?)?,
            None => return Some(cur_source.get("registry")?.as_str()?.to_string()),
        }
    }
}

/// Based on
/// https://github.com/rust-lang/cargo/blob/bb28e71202260180ecff658cd0fa0c7ba86d0296/src/cargo/sources/git/utils.rs#L344
/// and
/// https://github.com/rust-lang/cargo/blob/5102de2b7de997b03181063417f20874a06a67c0/src/cargo/sources/git/utils.rs#L644
fn with_authentication<T, F>(url: &str, mut f: F) -> T
    where F: FnMut(&mut git2::Credentials) -> T
{
    let cfg = GitConfig::open_default().unwrap();

    let mut cred_helper = git2::CredentialHelper::new(url);
    cred_helper.config(&cfg);
    f(&mut |url, username, allowed| if allowed.contains(CredentialType::SSH_KEY) {
        let user = username.map(|s| s.to_string())
            .or_else(|| cred_helper.username.clone())
            .unwrap_or("git".to_string());
        GitCred::ssh_key_from_agent(&user)
    } else if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
        GitCred::credential_helper(&cfg, url, username)
    } else if allowed.contains(CredentialType::DEFAULT) {
        GitCred::default()
    } else {
        Err(GitError::from_str("no authentication available")).unwrap()
    })
}


/// Find package data in the specified cargo index tree.
pub fn find_package_data<'t>(cratename: &str, registry: &Tree<'t>, registry_parent: &'t Repository) -> Option<Vec<u8>> {
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

    let ent = registry.get_name(elems[0])?;
    let obj = ent.to_object(registry_parent).ok()?;
    let ent = obj.as_tree()?.get_name(elems[1])?;
    let obj = ent.to_object(registry_parent).ok()?;
    if elems.len() == 3 {
        let ent = obj.as_tree()?.get_name(elems[2])?;
        let obj = ent.to_object(registry_parent).ok()?;
        Some(obj.as_blob()?.content().into())
    } else {
        Some(obj.as_blob()?.content().into())
    }
}

/// Check if there's a proxy specified to be used.
///
/// Look for `http.proxy` key in the `config` file parallel to the specified crates file.
///
/// Then look for `git`'s `http.proxy`.
///
/// Then for the `http_proxy`, `HTTP_PROXY`, `https_proxy`, and `HTTPS_PROXY` environment variables, in that order.
///
/// Based on Cargo's [`http_proxy_exists()` and
/// `http_proxy()`](https://github.com/rust-lang/cargo/blob/eebd1da3a89e9c7788d109b3e615e1e25dc2cfcd/src/cargo/ops/registry.rs)
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::find_proxy;
/// # use std::env::temp_dir;
/// # let crates_file = temp_dir().join(".crates.toml");
/// match find_proxy(&crates_file) {
///     Some(proxy) => println!("Proxy found at {}", proxy),
///     None => println!("No proxy detected"),
/// }
/// ```
pub fn find_proxy(crates_file: &Path) -> Option<String> {
    let config_file = crates_file.with_file_name("config");
    if config_file.exists() {
        let mut crates = String::new();
        File::open(&config_file).unwrap().read_to_string(&mut crates).unwrap();

        if let Some(proxy) = toml::from_str::<toml::Value>(&crates)
            .unwrap()
            .get("http")
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("proxy"))
            .and_then(|t| t.as_str()) {
            return Some(proxy.to_string());
        }
    }

    if let Ok(cfg) = GitConfig::open_default() {
        if let Ok(s) = cfg.get_str("http.proxy") {
            return Some(s.to_string());
        }
    }

    ["http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY"].iter().flat_map(env::var).next()
}

/// Find the bare git repository in the specified directory for the specified crate
///
/// The db directory is usually `$HOME/.cargo/git/db/`
///
/// The resulting paths are children of this directory in the format `cratename-hash`
pub fn find_git_db_repo(git_db_dir: &Path, cratename: &str) -> Option<PathBuf> {
    fs::read_dir(git_db_dir).ok()?.flatten().find(|de| de.file_name().to_str().map(|n| n.starts_with(cratename)).unwrap_or(false)).map(|de| de.path())
}
