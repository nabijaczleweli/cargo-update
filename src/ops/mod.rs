//! Main functions doing actual work.
//!
//! Use `installed_registry_packages()` to list the installed packages,
//! then use `intersect_packages()` to confirm which ones should be updated,
//! poll the packages' latest versions by calling `RegistryPackage::pull_version()` on them,
//! continue with doing whatever you wish.


use git2::{self, Config as GitConfig, Error as GitError, Cred as GitCred, RemoteCallbacks, CredentialType, FetchOptions, ProxyOptions, Repository, Tree, Oid};
use semver::{VersionReq as SemverReq, Version as Semver};
use std::io::{ErrorKind as IoErrorKind, Write, Read};
use std::ffi::{OsString, OsStr};
use std::collections::BTreeMap;
use std::path::{PathBuf, Path};
use std::hash::{Hasher, Hash};
use std::{cmp, env, mem, fs};
use std::process::Command;
use std::borrow::Cow;
use regex::Regex;
use url::Url;
use toml;
use json;
use hex;

mod config;

pub use self::config::*;


lazy_static! {
    static ref REGISTRY_RGX: Regex = Regex::new(r"([^\s]+) ([^\s]+) \(registry+\+([^\s]+)\)").unwrap();
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
/// # use cargo_update::ops::RegistryPackage;
/// # use semver::Version as Semver;
/// # fn main() {
/// let package_s = "racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)";
/// let mut package = RegistryPackage::parse(package_s, vec!["racer.exe".to_string()]).unwrap();
/// assert_eq!(package,
///            RegistryPackage {
///                name: "racer".to_string(),
///                registry: "https://github.com/rust-lang/crates.io-index".to_string(),
///                version: Some(Semver::parse("1.2.10").unwrap()),
///                newest_version: None,
///                alternative_version: None,
///                max_version: None,
///                executables: vec!["racer.exe".to_string()],
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
pub struct RegistryPackage {
    /// The package's name.
    ///
    /// Go to `https://crates.io/crates/{name}` to get the crate info, if available on the main repository.
    pub name: String,
    /// The registry the package is available from.
    ///
    /// Can be a name from ~/.cargo/config.
    ///
    /// The main repository is `https://github.com/rust-lang/crates.io-index`
    pub registry: String,
    /// The package's locally installed version.
    pub version: Option<Semver>,
    /// The latest version of the package, available at [`crates.io`](https://crates.io), if in main repository.
    ///
    /// `None` by default, acquire via `RegistryPackage::pull_version()`.
    pub newest_version: Option<Semver>,
    /// If present, the alternative newest version not chosen because of unfulfilled requirements like (not) being a prerelease.
    pub alternative_version: Option<Semver>,
    /// User-bounded maximum version to update up to.
    pub max_version: Option<Semver>,
    /// Executables currently installed for this package.
    pub executables: Vec<String>,
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
/// let mut package = GitRepoPackage::parse(package_s, vec!["alacritty".to_string()]).unwrap();
/// assert_eq!(package,
///            GitRepoPackage {
///                name: "alacritty".to_string(),
///                url: "https://github.com/jwilm/alacritty".to_string(),
///                branch: None,
///                id: git2::Oid::from_str("eb231b3e70b87875df4bdd1974d5e94704024d70").unwrap(),
///                newest_id: None,
///                executables: vec!["alacritty".to_string()],
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
    /// The latest version of the package available at the main [`crates.io`](https://crates.io) repository.
    ///
    /// `None` by default, acquire via `GitRepoPackage::pull_version()`.
    pub newest_id: Option<Oid>,
    /// Executables currently installed for this package.
    pub executables: Vec<String>,
}


impl RegistryPackage {
    /// Try to decypher a package descriptor into a `RegistryPackage`.
    ///
    /// Will return `None` if the given package descriptor is invalid.
    ///
    /// In the returned instance, `newest_version` is always `None`, get it via `RegistryPackage::pull_version()`.
    ///
    /// The executable list is used as-is.
    ///
    /// # Examples
    ///
    /// Main repository packages:
    ///
    /// ```
    /// # extern crate cargo_update;
    /// # extern crate semver;
    /// # use cargo_update::ops::RegistryPackage;
    /// # use semver::Version as Semver;
    /// # fn main() {
    /// let package_s = "racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)";
    /// assert_eq!(RegistryPackage::parse(package_s, vec!["racer.exe".to_string()]).unwrap(),
    ///            RegistryPackage {
    ///                name: "racer".to_string(),
    ///                registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///                version: Some(Semver::parse("1.2.10").unwrap()),
    ///                newest_version: None,
    ///                alternative_version: None,
    ///                max_version: None,
    ///                executables: vec!["racer.exe".to_string()],
    ///            });
    ///
    /// let package_s = "cargo-outdated 0.2.0 (registry+file:///usr/local/share/cargo)";
    /// assert_eq!(RegistryPackage::parse(package_s, vec!["cargo-outdated".to_string()]).unwrap(),
    ///            RegistryPackage {
    ///                name: "cargo-outdated".to_string(),
    ///                registry: "file:///usr/local/share/cargo".to_string(),
    ///                version: Some(Semver::parse("0.2.0").unwrap()),
    ///                newest_version: None,
    ///                alternative_version: None,
    ///                max_version: None,
    ///                executables: vec!["cargo-outdated".to_string()],
    ///            });
    /// # }
    /// ```
    ///
    /// Git repository:
    ///
    /// ```
    /// # use cargo_update::ops::RegistryPackage;
    /// let package_s = "treesize 0.2.1 (git+https://github.com/melak47/treesize-rs#v0.2.1)";
    /// assert!(RegistryPackage::parse(package_s, vec!["treesize".to_string()]).is_none());
    /// ```
    pub fn parse(what: &str, executables: Vec<String>) -> Option<RegistryPackage> {
        REGISTRY_RGX.captures(what).map(|c| {
            RegistryPackage {
                name: c.get(1).unwrap().as_str().to_string(),
                registry: c.get(3).unwrap().as_str().to_string(),
                version: Some(Semver::parse(c.get(2).unwrap().as_str()).unwrap()),
                newest_version: None,
                alternative_version: None,
                max_version: None,
                executables: executables,
            }
        })
    }

    /// Download the version list for this crate off the specified repository tree and set the latest and alternative versions.
    pub fn pull_version<'t>(&mut self, registry: &Tree<'t>, registry_parent: &'t Repository, install_prereleases: Option<bool>) {
        let mut vers =
            crate_versions(&mut &find_package_data(&self.name, registry, registry_parent).ok_or_else(|| format!("package {} not found", self.name)).unwrap()
                                     [..]);
        vers.sort();

        self.newest_version = None;
        self.alternative_version = None;

        let mut vers = vers.into_iter().rev();
        if let Some(newest) = vers.next() {
            self.newest_version = Some(newest);

            if self.newest_version.as_ref().unwrap().is_prerelease() && !install_prereleases.unwrap_or(false) {
                if let Some(newest_nonpre) = vers.find(|v| !v.is_prerelease()) {
                    mem::swap(&mut self.alternative_version, &mut self.newest_version);
                    self.newest_version = Some(newest_nonpre);
                }
            }
        }
    }

    /// Check whether this package needs to be installed
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cargo_update;
    /// # extern crate semver;
    /// # use semver::{VersionReq as SemverReq, Version as Semver};
    /// # use cargo_update::ops::RegistryPackage;
    /// # use std::str::FromStr;
    /// # fn main() {
    /// assert!(RegistryPackage {
    ///             name: "racer".to_string(),
    ///             registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///             version: Some(Semver::parse("1.7.2").unwrap()),
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///             alternative_version: None,
    ///             max_version: None,
    ///             executables: vec!["racer".to_string()],
    ///         }.needs_update(None, None, false));
    /// assert!(RegistryPackage {
    ///             name: "racer".to_string(),
    ///             registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///             version: None,
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///             alternative_version: None,
    ///             max_version: None,
    ///             executables: vec!["racer".to_string()],
    ///         }.needs_update(None, None, false));
    /// assert!(RegistryPackage {
    ///             name: "racer".to_string(),
    ///             registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///             version: Some(Semver::parse("2.0.7").unwrap()),
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///             alternative_version: None,
    ///             max_version: None,
    ///             executables: vec!["racer".to_string()],
    ///         }.needs_update(None, None, true));
    /// assert!(!RegistryPackage {
    ///             name: "racer".to_string(),
    ///             registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///             version: Some(Semver::parse("2.0.6").unwrap()),
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///             alternative_version: None,
    ///             max_version: None,
    ///             executables: vec!["racer".to_string()],
    ///         }.needs_update(None, None, false));
    /// assert!(!RegistryPackage {
    ///             name: "racer".to_string(),
    ///             registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///             version: Some(Semver::parse("2.0.6").unwrap()),
    ///             newest_version: None,
    ///             alternative_version: None,
    ///             max_version: None,
    ///             executables: vec!["racer".to_string()],
    ///         }.needs_update(None, None, false));
    ///
    /// let req = SemverReq::from_str("^1.7").unwrap();
    /// assert!(RegistryPackage {
    ///             name: "racer".to_string(),
    ///             registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///             version: Some(Semver::parse("1.7.2").unwrap()),
    ///             newest_version: Some(Semver::parse("1.7.3").unwrap()),
    ///             alternative_version: None,
    ///             max_version: None,
    ///             executables: vec!["racer".to_string()],
    ///         }.needs_update(Some(&req), None, false));
    /// assert!(RegistryPackage {
    ///             name: "racer".to_string(),
    ///             registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///             version: None,
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///             alternative_version: None,
    ///             max_version: None,
    ///             executables: vec!["racer".to_string()],
    ///         }.needs_update(Some(&req), None, false));
    /// assert!(!RegistryPackage {
    ///             name: "racer".to_string(),
    ///             registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///             version: Some(Semver::parse("1.7.2").unwrap()),
    ///             newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///             alternative_version: None,
    ///             max_version: None,
    ///             executables: vec!["racer".to_string()],
    ///         }.needs_update(Some(&req), None, false));
    ///
    /// assert!(!RegistryPackage {
    ///             name: "cargo-audit".to_string(),
    ///             registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///             version: None,
    ///             newest_version: Some(Semver::parse("0.9.0-beta2").unwrap()),
    ///             alternative_version: None,
    ///             max_version: None,
    ///             executables: vec!["racer".to_string()],
    ///         }.needs_update(Some(&req), None, false));
    /// assert!(RegistryPackage {
    ///             name: "cargo-audit".to_string(),
    ///             registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///             version: None,
    ///             newest_version: Some(Semver::parse("0.9.0-beta2").unwrap()),
    ///             alternative_version: None,
    ///             max_version: None,
    ///             executables: vec!["racer".to_string()],
    ///         }.needs_update(Some(&req), Some(true), false));
    /// # }
    /// ```
    pub fn needs_update(&self, req: Option<&SemverReq>, install_prereleases: Option<bool>, downdate: bool) -> bool {
        fn criterion(fromver: &Semver, tover: &Semver, downdate: bool) -> bool {
            if downdate {
                fromver != tover
            } else {
                fromver < tover
            }
        }

        let update_to_version = self.update_to_version();

        (req.into_iter().zip(self.version.as_ref()).map(|(sr, cv)| !sr.matches(cv)).next().unwrap_or(true) ||
         req.into_iter().zip(update_to_version).map(|(sr, uv)| sr.matches(uv)).next().unwrap_or(true)) &&
        update_to_version.map(|upd_v| {
                (!upd_v.is_prerelease() || install_prereleases.unwrap_or(false)) &&
                (self.version.is_none() || criterion(self.version.as_ref().unwrap(), upd_v, downdate))
            })
            .unwrap_or(false)
    }

    /// Get package version to update to, or `None` if the crate has no newest version (was yanked)
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cargo_update;
    /// # extern crate semver;
    /// # use cargo_update::ops::RegistryPackage;
    /// # use semver::Version as Semver;
    /// # fn main() {
    /// assert_eq!(RegistryPackage {
    ///                name: "racer".to_string(),
    ///                registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///                version: Some(Semver::parse("1.7.2").unwrap()),
    ///                newest_version: Some(Semver::parse("2.0.6").unwrap()),
    ///                alternative_version: None,
    ///                max_version: Some(Semver::parse("2.0.5").unwrap()),
    ///                executables: vec!["racer".to_string()],
    ///            }.update_to_version(),
    ///            Some(&Semver::parse("2.0.5").unwrap()));
    /// assert_eq!(RegistryPackage {
    ///                name: "gutenberg".to_string(),
    ///                registry: "https://github.com/rust-lang/crates.io-index".to_string(),
    ///                version: Some(Semver::parse("0.0.7").unwrap()),
    ///                newest_version: None,
    ///                alternative_version: None,
    ///                max_version: None,
    ///                executables: vec!["gutenberg".to_string()],
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
    ///   * the package descriptor is not from a git repository.
    ///
    /// In the returned instance, `newest_version` is always `None`, get it via `GitRepoPackage::pull_version()`.
    ///
    /// The executable list is used as-is.
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
    /// assert_eq!(GitRepoPackage::parse(package_s, vec!["alacritty".to_string()]).unwrap(),
    ///            GitRepoPackage {
    ///                name: "alacritty".to_string(),
    ///                url: "https://github.com/jwilm/alacritty".to_string(),
    ///                branch: None,
    ///                id: git2::Oid::from_str("eb231b3e70b87875df4bdd1974d5e94704024d70").unwrap(),
    ///                newest_id: None,
    ///                executables: vec!["alacritty".to_string()],
    ///            });
    ///
    /// let package_s = "chattium-oxide-client 0.1.0 \
    ///                  (git+https://github.com/nabijaczleweli/chattium-oxide-client\
    ///                       ?branch=master#108a7b94f0e0dcb2a875f70fc0459d5a682df14c)";
    /// assert_eq!(GitRepoPackage::parse(package_s, vec!["chattium-oxide-client.exe".to_string()]).unwrap(),
    ///            GitRepoPackage {
    ///                name: "chattium-oxide-client".to_string(),
    ///                url: "https://github.com/nabijaczleweli/chattium-oxide-client".to_string(),
    ///                branch: Some("master".to_string()),
    ///                id: git2::Oid::from_str("108a7b94f0e0dcb2a875f70fc0459d5a682df14c").unwrap(),
    ///                newest_id: None,
    ///                executables: vec!["chattium-oxide-client.exe".to_string()],
    ///            });
    /// # }
    /// ```
    ///
    /// Main repository package:
    ///
    /// ```
    /// # use cargo_update::ops::GitRepoPackage;
    /// let package_s = "racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)";
    /// assert!(GitRepoPackage::parse(package_s, vec!["racer".to_string()]).is_none());
    /// ```
    pub fn parse(what: &str, executables: Vec<String>) -> Option<GitRepoPackage> {
        GIT_PACKAGE_RGX.captures(what).map(|c| {
            let mut url = Url::parse(c.get(3).unwrap().as_str()).unwrap();
            let branch = url.query_pairs().find(|&(ref name, _)| name == "branch").map(|(_, value)| value.to_string());
            url.set_query(None);
            GitRepoPackage {
                name: c.get(1).unwrap().as_str().to_string(),
                url: url.into(),
                branch: branch,
                id: Oid::from_str(c.get(4).unwrap().as_str()).unwrap(),
                newest_id: None,
                executables: executables,
            }
        })
    }

    /// Clone the repo and check what the latest commit's hash is.
    pub fn pull_version<Pt: AsRef<Path>, Pg: AsRef<Path>>(&mut self, temp_dir: Pt, git_db_dir: Pg, http_proxy: Option<&str>, fork_git: bool) {
        self.pull_version_impl(temp_dir.as_ref(), git_db_dir.as_ref(), http_proxy, fork_git)
    }

    fn pull_version_impl(&mut self, temp_dir: &Path, git_db_dir: &Path, http_proxy: Option<&str>, fork_git: bool) {
        let clone_dir = find_git_db_repo(git_db_dir, &self.url).unwrap_or_else(|| {
            fs::create_dir_all(temp_dir).unwrap();
            temp_dir.join(&self.name)
        });

        let repo = self.pull_version_repo(&clone_dir, http_proxy, fork_git);

        self.newest_id = Some(repo.and_then(|r| r.head().and_then(|h| h.target().ok_or_else(|| GitError::from_str("HEAD not a direct reference")))).unwrap());
    }

    fn pull_version_fresh_clone(&self, clone_dir: &Path, http_proxy: Option<&str>, fork_git: bool) -> Result<Repository, GitError> {
        if fork_git {
            Command::new(env::var_os("GIT").as_ref().map(OsString::as_os_str).unwrap_or(OsStr::new("git")))
                .arg("clone")
                .args(self.branch.as_ref().map(|_| "-b"))
                .args(self.branch.as_ref())
                .args(&["--bare", "--", &self.url])
                .arg(clone_dir)
                .status()
                .map_err(|e| GitError::from_str(&e.to_string()))
                .and_then(|e| if e.success() {
                    Repository::open(clone_dir)
                } else {
                    Err(GitError::from_str(&e.to_string()))
                })
        } else {
            with_authentication(&self.url, |creds| {
                let mut bldr = git2::build::RepoBuilder::new();

                let mut cb = RemoteCallbacks::new();
                cb.credentials(|a, b, c| creds(a, b, c));
                bldr.fetch_options(fetch_options_from_proxy_url_and_callbacks(&self.url, http_proxy, cb));
                if let Some(ref b) = self.branch.as_ref() {
                    bldr.branch(b);
                }

                bldr.bare(true);
                bldr.clone(&self.url, &clone_dir)
            })
        }
    }

    fn pull_version_repo(&self, clone_dir: &Path, http_proxy: Option<&str>, fork_git: bool) -> Result<Repository, GitError> {
        if let Ok(r) = Repository::open(clone_dir) {
            // If `Repository::open` is successful, both `clone_dir` exists *and* points to a valid repository.
            //
            // Fetch the specified or default branch, reset it to the remote HEAD.

            let (branch, tofetch) = match self.branch.as_ref() {
                Some(b) => {
                    // Cargo doesn't point the HEAD at the chosen (via "--branch") branch when installing
                    // https://github.com/nabijaczleweli/cargo-update/issues/143
                    r.set_head(&format!("refs/heads/{}", b)).map_err(|e| panic!("Couldn't set HEAD to chosen branch {}: {}", b, e)).unwrap();
                    (Cow::from(b), Cow::from(b))
                }

                None => {
                    match r.find_reference("HEAD")
                        .map_err(|e| panic!("No HEAD in {}: {}", clone_dir.display(), e))
                        .unwrap()
                        .symbolic_target() {
                        Some(ht) => (ht["refs/heads/".len()..].to_string().into(), "+HEAD:refs/remotes/origin/HEAD".into()),
                        None => {
                            // Versions up to v4.0.0 (well, 59be1c0de283dabce320a860a3d533d00910a6a9, but who's counting)
                            // called r.set_head("FETCH_HEAD"), which made HEAD a direct SHA reference.
                            // This is obviously problematic when trying to read the default branch, and these checkouts can persist
                            // (https://github.com/nabijaczleweli/cargo-update/issues/139#issuecomment-665847290);
                            // yeeting them shouldn't be a problem, since that's what we *would* do anyway,
                            // and we set up for the non-pessimised path in later runs.
                            fs::remove_dir_all(clone_dir).unwrap();
                            return self.pull_version_fresh_clone(clone_dir, http_proxy, fork_git);
                        }
                    }

                }
            };

            let mut remote = "origin";
            r.find_remote("origin")
                .or_else(|_| {
                    remote = &self.url;
                    r.remote_anonymous(&self.url)
                })
                .and_then(|mut rm| if fork_git {
                    Command::new(env::var_os("GIT").as_ref().map(OsString::as_os_str).unwrap_or(OsStr::new("git")))
                        .arg("-C")
                        .arg(r.path())
                        .args(&["fetch", remote, &tofetch])
                        .status()
                        .map_err(|e| GitError::from_str(&e.to_string()))
                        .and_then(|e| if e.success() {
                            Ok(())
                        } else {
                            Err(GitError::from_str(&e.to_string()))
                        })
                } else {
                    with_authentication(&self.url, |creds| {
                        let mut cb = RemoteCallbacks::new();
                        cb.credentials(|a, b, c| creds(a, b, c));

                        rm.fetch(&[&tofetch[..]],
                                 Some(&mut fetch_options_from_proxy_url_and_callbacks(&self.url, http_proxy, cb)),
                                 None)
                    })
                })
                .map_err(|e| panic!("Fetching {} from {}: {}", clone_dir.display(), self.url, e))
                .unwrap();
            r.branch(&branch,
                        &r.find_reference("FETCH_HEAD")
                            .map_err(|e| panic!("No FETCH_HEAD in {}: {}", clone_dir.display(), e))
                            .unwrap()
                            .peel_to_commit()
                            .map_err(|e| panic!("FETCH_HEAD not a commit in {}: {}", clone_dir.display(), e))
                            .unwrap(),
                        true)
                .map_err(|e| panic!("Setting local branch {} in {}: {}", branch, clone_dir.display(), e))
                .unwrap();
            Ok(r)
        } else {
            // If we could not open the repository either it does not exist, or exists but is invalid,
            // in which case remove it to trigger a fresh clone.
            if clone_dir.exists() {
                fs::remove_dir_all(&clone_dir).unwrap();
            }

            self.pull_version_fresh_clone(clone_dir, http_proxy, fork_git)
        }
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
    ///             executables: vec!["alacritty".to_string()],
    ///         }.needs_update());
    /// assert!(!GitRepoPackage {
    ///             name: "alacritty".to_string(),
    ///             url: "https://github.com/jwilm/alacritty".to_string(),
    ///             branch: None,
    ///             id: git2::Oid::from_str("5f7885749c4d7e48869b1fc0be4d430601cdbbfa").unwrap(),
    ///             newest_id: Some(git2::Oid::from_str("5f7885749c4d7e48869b1fc0be4d430601cdbbfa").unwrap()),
    ///             executables: vec!["alacritty".to_string()],
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


/// `cargo` configuration, as obtained from `.cargo/config[.toml]`
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct CargoConfig {
    pub net_git_fetch_with_cli: bool,
}

impl CargoConfig {
    pub fn load(crates_file: &Path) -> CargoConfig {
        CargoConfig {
            net_git_fetch_with_cli: env::var("CARGO_NET_GIT_FETCH_WITH_CLI")
                .ok()
                .and_then(|e| if e.is_empty() {
                    Some(toml::Value::String(String::new()))
                } else {
                    e.parse::<toml::Value>().ok()
                })
                .or_else(|| {
                    fs::read_to_string(crates_file.with_file_name("config"))
                        .or_else(|_| fs::read_to_string(crates_file.with_file_name("config.toml")))
                        .ok()?
                        .parse::<toml::Value>()
                        .ok()?
                        .as_table_mut()?
                        .remove("net")?
                        .as_table_mut()?
                        .remove("git-fetch-with-cli")
                })
                .map(CargoConfig::truthy)
                .unwrap_or(false),
        }
    }

    fn truthy(v: toml::Value) -> bool {
        match v {
            toml::Value::String(ref s) if s == "" => false,
            toml::Value::Float(f) if f == 0. => false,
            toml::Value::Integer(0) |
            toml::Value::Boolean(false) => false,
            _ => true,
        }
    }
}


/// [Follow `install.root`](https://github.com/nabijaczleweli/cargo-update/issues/23) in the `config` or `config.toml` file
/// in the cargo directory specified.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::crates_file_in;
/// # use std::env::temp_dir;
/// # let cargo_dir = temp_dir();
/// let cargo_dir = crates_file_in(&cargo_dir);
/// # let _ = cargo_dir;
/// ```
pub fn crates_file_in(cargo_dir: &Path) -> PathBuf {
    let mut config_file = cargo_dir.join("config");
    if !config_file.exists() {
        config_file.set_file_name("config.toml");
    }
    if config_file.exists() {
        if let Some(idir) = toml::from_str::<toml::Value>(&fs::read_to_string(&config_file).unwrap())
            .unwrap()
            .get("install")
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("root"))
            .and_then(|t| t.as_str()) {
            return crates_file_in(Path::new(idir));
        }
    }

    config_file.set_file_name(".crates.toml");
    config_file
}

/// List the installed packages at the specified location that originate
/// from the a cargo registry.
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
/// # use cargo_update::ops::installed_registry_packages;
/// # use std::env::temp_dir;
/// # let cargo_dir = temp_dir().join(".crates.toml");
/// let packages = installed_registry_packages(&cargo_dir);
/// for package in &packages {
///     println!("{} v{}", package.name, package.version.as_ref().unwrap());
/// }
/// ```
pub fn installed_registry_packages(crates_file: &Path) -> Vec<RegistryPackage> {
    if crates_file.exists() {
        let mut res = Vec::<RegistryPackage>::new();
        for pkg in match toml::from_str::<toml::Value>(&fs::read_to_string(crates_file).unwrap()).unwrap().get("v1") {
                Some(tbl) => tbl,
                None => return Vec::new(),
            }
            .as_table()
            .unwrap()
            .iter()
            .flat_map(|(s, x)| x.as_array().map(|x| (s, x)))
            .flat_map(|(s, x)| RegistryPackage::parse(s, x.iter().flat_map(toml::Value::as_str).map(str::to_string).collect())) {
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
        let mut res = Vec::<GitRepoPackage>::new();
        for pkg in match toml::from_str::<toml::Value>(&fs::read_to_string(crates_file).unwrap()).unwrap().get("v1") {
                Some(tbl) => tbl,
                None => return Vec::new(),
            }
            .as_table()
            .unwrap()
            .iter()
            .flat_map(|(s, x)| x.as_array().map(|x| (s, x)))
            .flat_map(|(s, x)| GitRepoPackage::parse(s, x.iter().flat_map(toml::Value::as_str).map(str::to_string).collect())) {
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

/// Filter out the installed packages not specified to be updated and add the packages you specify to install,
/// if they aren't already installed via git.
///
/// List installed packages with `installed_registry_packages()`.
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::{RegistryPackage, intersect_packages};
/// # fn installed_registry_packages(_: &()) {}
/// # let cargo_dir = ();
/// # let packages_to_update = [("racer".to_string(), None,
/// #                            "registry+https://github.com/rust-lang/crates.io-index".to_string()),
/// #                           ("cargo-outdated".to_string(), None,
/// #                            "registry+https://github.com/rust-lang/crates.io-index".to_string())];
/// let mut installed_packages = installed_registry_packages(&cargo_dir);
/// # let mut installed_packages =
/// #     vec![RegistryPackage::parse("cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)",
/// #     vec!["cargo-outdated".to_string()]).unwrap(),
/// #          RegistryPackage::parse("racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)",
/// #     vec!["racer.exe".to_string()]).unwrap(),
/// #          RegistryPackage::parse("rustfmt 0.6.2 (registry+https://github.com/rust-lang/crates.io-index)",
/// #     vec!["rustfmt".to_string(), "cargo-format".to_string()]).unwrap()];
/// installed_packages = intersect_packages(&installed_packages, &packages_to_update, false, &[]);
/// # assert_eq!(&installed_packages,
/// #   &[RegistryPackage::parse("cargo-outdated 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)",
/// #                            vec!["cargo-outdated".to_string()]).unwrap(),
/// #     RegistryPackage::parse("racer 1.2.10 (registry+https://github.com/rust-lang/crates.io-index)",
/// #                            vec!["racer.exe".to_string()]).unwrap()]);
/// ```
pub fn intersect_packages(installed: &[RegistryPackage], to_update: &[(String, Option<Semver>, String)], allow_installs: bool,
                          installed_git: &[GitRepoPackage])
                          -> Vec<RegistryPackage> {
    installed.iter()
        .filter(|p| to_update.iter().any(|u| p.name == u.0))
        .cloned()
        .map(|p| RegistryPackage { max_version: to_update.iter().find(|u| p.name == u.0).and_then(|u| u.1.clone()), ..p })
        .chain(to_update.iter()
            .filter(|p| allow_installs && installed.iter().find(|i| i.name == p.0).is_none() && installed_git.iter().find(|i| i.name == p.0).is_none())
            .map(|p| {
                RegistryPackage {
                    name: p.0.clone(),
                    registry: p.2.clone(),
                    version: None,
                    newest_version: None,
                    alternative_version: None,
                    max_version: p.1.clone(),
                    executables: vec![],
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

/// Get the location of the registry index corresponding ot the given URL; if not present – make it and its parents.
///
/// As odd as it may be, this [can happen (if rarely) and is a supported
/// configuration](https://github.com/nabijaczleweli/cargo-update/issues/150).
///
/// # Examples
///
/// ```
/// # use cargo_update::ops::assert_index_path;
/// # use std::env::temp_dir;
/// # let cargo_dir = temp_dir().join("cargo_update-doctest").join("assert_index_path-0");
/// # let idx_dir = cargo_dir.join("registry").join("index").join("github.com-1ecc6299db9ec823");
/// let index = assert_index_path(&cargo_dir, "https://github.com/rust-lang/crates.io-index").unwrap();
///
/// // Use find_package_data() to look for packages
/// # assert_eq!(index, idx_dir);
/// ```
pub fn assert_index_path(cargo_dir: &Path, registry_url: &str) -> Result<PathBuf, Cow<'static, str>> {
    let path = cargo_dir.join("registry").join("index").join(registry_shortname(registry_url));
    match path.metadata() {
        Ok(meta) => {
            if meta.is_dir() {
                Ok(path)
            } else {
                Err(format!("{} (index directory for {}) not a directory", path.display(), registry_url).into())
            }
        }
        Err(ref e) if e.kind() == IoErrorKind::NotFound => {
            fs::create_dir_all(&path).map_err(|e| format!("Couldn't create {} (index directory for {}): {}", path.display(), registry_url, e))?;
            Ok(path)
        }
        Err(e) => Err(format!("Couldn't read {} (index directory for {}): {}", path.display(), registry_url, e).into()),
    }
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
/// and left garbage in the console, making it unsuitable.
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
///
/// Sometimes, however, even this isn't enough (see https://github.com/nabijaczleweli/cargo-update/issues/163),
/// hence `fork_git`, which actually runs `$GIT` (default: `git`).
pub fn update_index<W: Write>(index_repo: &mut Repository, repo_url: &str, http_proxy: Option<&str>, fork_git: bool, out: &mut W) -> Result<(), String> {
    writeln!(out, "    Updating registry '{}'", repo_url).map_err(|_| "failed to write updating message".to_string())?;
    if fork_git {
        Command::new(env::var_os("GIT").as_ref().map(OsString::as_os_str).unwrap_or(OsStr::new("git"))).arg("-C")
            .arg(index_repo.path())
            .args(&["fetch", "-f", repo_url, "HEAD:refs/remotes/origin/HEAD"])
            .status()
            .map_err(|e| e.to_string())
            .and_then(|e| if e.success() {
                Ok(())
            } else {
                Err(e.to_string())
            })?;
    } else {
        index_repo.remote_anonymous(repo_url)
            .and_then(|mut r| {
                with_authentication(repo_url, |creds| {
                    let mut cb = RemoteCallbacks::new();
                    cb.credentials(|a, b, c| creds(a, b, c));

                    r.fetch(&["HEAD:refs/remotes/origin/HEAD"],
                            Some(&mut fetch_options_from_proxy_url_and_callbacks(repo_url, http_proxy, cb)),
                            None)
                })
            })
            .map_err(|e| e.message().to_string())?;
    }
    writeln!(out).map_err(|_| "failed to write post-update newline".to_string())?;

    Ok(())
}

fn fetch_options_from_proxy_url_and_callbacks<'a>(repo_url: &str, proxy_url: Option<&str>, callbacks: RemoteCallbacks<'a>) -> FetchOptions<'a> {
    let mut ret = FetchOptions::new();
    if let Some(proxy_url) = proxy_url {
        ret.proxy_options({
            let mut prx = ProxyOptions::new();
            let mut url = Cow::from(proxy_url);

            // Cargo allows [protocol://]host[:port], but git needs the protocol, try to crudely add it here if missing;
            // confer https://github.com/nabijaczleweli/cargo-update/issues/144.
            if Url::parse(proxy_url).is_err() {
                if let Ok(rurl) = Url::parse(repo_url) {
                    let replacement_proxy_url = format!("{}://{}", rurl.scheme(), proxy_url);
                    if Url::parse(&replacement_proxy_url).is_ok() {
                        url = Cow::from(replacement_proxy_url);
                    }
                }
            }

            prx.url(&url);
            prx
        });
    }
    ret.remote_callbacks(callbacks);
    ret
}

/// Get the URL to update index from and the cargo name for it from the config file parallel to the specified crates file
///
/// First gets the source name corresponding to the given URL, if appropriate,
/// then chases the `source.$SRCNAME.replace-with` chain,
/// then retrieves the URL from `source.$SRCNAME.registry` of the final source.
///
/// Prepopulates with `source.crates-io.registry = "https://github.com/rust-lang/crates.io-index"`,
/// as specified in the book
///
/// Consult [#107](https://github.com/nabijaczleweli/cargo-update/issues/107) and
/// the Cargo Book for details: https://doc.rust-lang.org/cargo/reference/source-replacement.html,
/// https://doc.rust-lang.org/cargo/reference/registries.html.
pub fn get_index_url(crates_file: &Path, registry: &str) -> Result<(String, Cow<'static, str>), Cow<'static, str>> {
    let mut config_file = crates_file.with_file_name("config");
    let config = if let Ok(cfg) = fs::read_to_string(&config_file).or_else(|_| {
        config_file.set_file_name("config.toml");
        fs::read_to_string(&config_file)
    }) {
        toml::from_str::<toml::Value>(&cfg).map_err(|e| format!("{} not TOML: {}", config_file.display(), e))?
    } else {
        if registry == "https://github.com/rust-lang/crates.io-index" {
            return Ok((registry.to_string(), "crates-io".into()));
        } else {
            Err(format!("Non-crates.io registry specified and no config file found at {} or {}. \
                         Due to a Cargo limitation we will not be able to install from there \
                         until it's given a [source.NAME] in that file!",
                        config_file.with_file_name("config").display(),
                        config_file.display()))?
        }
    };

    let mut replacements = BTreeMap::new();
    let mut registries = BTreeMap::new();
    let mut cur_source = Cow::from(registry);

    // Special case, always present
    registries.insert("crates-io", Cow::from("https://github.com/rust-lang/crates.io-index"));
    if cur_source == "https://github.com/rust-lang/crates.io-index" {
        cur_source = "crates-io".into();
    }

    if let Some(source) = config.get("source") {
        for (name, v) in source.as_table().ok_or("source not table")? {
            if let Some(replacement) = v.get("replace-with") {
                replacements.insert(&name[..],
                                    replacement.as_str().ok_or_else(|| format!("source.{}.replacement not string", name))?);
            }

            if let Some(url) = v.get("registry") {
                let url = url.as_str().ok_or_else(|| format!("source.{}.registry not string", name))?.to_string().into();
                if cur_source == url {
                    cur_source = name.into();
                }

                registries.insert(&name[..], url);
            }
        }
    }

    if let Some(registries_tabls) = config.get("registries") {
        let table = registries_tabls.as_table().ok_or("registries is not a table")?;
        for (name, url) in table.iter().flat_map(|(name, val)| val.as_table()?.get("index")?.as_str().map(|v| (name, v))) {
            if cur_source == url {
                cur_source = name.into()
            }
            registries.insert(name, url.into());
        }
    }

    if Url::parse(&cur_source).is_ok() {
        Err(format!("Non-crates.io registry specified and {} couldn't be found in the config file at {}. \
                     Due to a Cargo limitation we will not be able to install from there \
                     until it's given a [source.NAME] in that file!",
                    cur_source,
                    config_file.display()))?
    }

    while let Some(repl) = replacements.get(&cur_source[..]) {
        cur_source = Cow::from(&repl[..]);
    }

    registries.get(&cur_source[..]).map(|reg| (reg.to_string(), cur_source.to_string().into())).ok_or_else(|| {
        format!("Couldn't find appropriate source URL for {} in {} (resolved to {:?})",
                registry,
                config_file.display(),
                cur_source)
            .into()
    })
}

/// Based on
/// https://github.com/rust-lang/cargo/blob/bb28e71202260180ecff658cd0fa0c7ba86d0296/src/cargo/sources/git/utils.rs#L344
/// and
/// https://github.com/rust-lang/cargo/blob/5102de2b7de997b03181063417f20874a06a67c0/src/cargo/sources/git/utils.rs#L644,
/// then
/// https://github.com/rust-lang/cargo/blob/5102de2b7de997b03181063417f20874a06a67c0/src/cargo/sources/git/utils.rs#L437
/// (see that link for full comments)
fn with_authentication<T, F>(url: &str, mut f: F) -> Result<T, GitError>
    where F: FnMut(&mut git2::Credentials) -> Result<T, GitError>
{
    let cfg = GitConfig::open_default().unwrap();

    let mut cred_helper = git2::CredentialHelper::new(url);
    cred_helper.config(&cfg);

    let mut ssh_username_requested = false;
    let mut cred_helper_bad = None;
    let mut ssh_agent_attempts = Vec::new();
    let mut any_attempts = false;
    let mut tried_ssh_key = false;

    let mut res = f(&mut |url, username, allowed| {
        any_attempts = true;

        if allowed.contains(CredentialType::USERNAME) {
            ssh_username_requested = true;

            Err(GitError::from_str("username to be tried later"))
        } else if allowed.contains(CredentialType::SSH_KEY) && !tried_ssh_key {
            tried_ssh_key = true;

            let username = username.unwrap();
            ssh_agent_attempts.push(username.to_string());

            GitCred::ssh_key_from_agent(username)
        } else if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) && cred_helper_bad.is_none() {
            let ret = GitCred::credential_helper(&cfg, url, username);
            cred_helper_bad = Some(ret.is_err());
            ret
        } else if allowed.contains(CredentialType::DEFAULT) {
            GitCred::default()
        } else {
            Err(GitError::from_str("no authentication available"))
        }
    });

    if ssh_username_requested {
        // NOTE: this is the only divergence from the original cargo code: we also try cfg["user.name"]
        //       see https://github.com/nabijaczleweli/cargo-update/issues/110#issuecomment-533091965 for explanation
        for uname in cred_helper.username
            .into_iter()
            .chain(cfg.get_string("user.name"))
            .chain(["USERNAME", "USER"].iter().flat_map(env::var))
            .chain(Some("git").into_iter().map(str::to_string)) {
            let mut ssh_attempts = 0;

            res = f(&mut |_, _, allowed| {
                if allowed.contains(CredentialType::USERNAME) {
                    return GitCred::username(&uname);
                } else if allowed.contains(CredentialType::SSH_KEY) {
                    ssh_attempts += 1;
                    if ssh_attempts == 1 {
                        ssh_agent_attempts.push(uname.to_string());
                        return GitCred::ssh_key_from_agent(&uname);
                    }
                }

                Err(GitError::from_str("no authentication available"))
            });

            if ssh_attempts != 2 {
                break;
            }
        }
    }

    if res.is_ok() || !any_attempts {
        res
    } else {
        let err = res.err().map(|e| format!("{}: ", e)).unwrap_or_default();

        let mut msg = format!("{}failed to authenticate when downloading repository {}", err, url);
        if !ssh_agent_attempts.is_empty() {
            msg.push_str(" (tried ssh-agent, but none of the following usernames worked: ");
            for (i, uname) in ssh_agent_attempts.into_iter().enumerate() {
                if i != 0 {
                    msg.push_str(", ");
                }
                msg.push('\"');
                msg.push_str(&uname);
                msg.push('\"');
            }
            msg.push(')');
        }

        if let Some(failed_cred_helper) = cred_helper_bad {
            msg.push_str(" (tried to find username+password via ");
            if failed_cred_helper {
                msg.push_str("git's credential.helper support, but failed)");
            } else {
                msg.push_str("credential.helper, but found credentials were incorrect)");
            }
        }

        Err(GitError::from_str(&msg)).unwrap()
    }
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
/// If a proxy is specified, but an empty string, treat it as unspecified.
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
        if let Some(proxy) = toml::from_str::<toml::Value>(&fs::read_to_string(config_file).unwrap())
            .unwrap()
            .get("http")
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("proxy"))
            .and_then(|t| t.as_str()) {
            if !proxy.is_empty() {
                return Some(proxy.to_string());
            }
        }
    }

    if let Ok(cfg) = GitConfig::open_default() {
        if let Ok(proxy) = cfg.get_str("http.proxy") {
            if !proxy.is_empty() {
                return Some(proxy.to_string());
            }
        }
    }

    ["http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY"].iter().flat_map(env::var).filter(|proxy| !proxy.is_empty()).next()
}

/// Find the bare git repository in the specified directory for the specified crate
///
/// The db directory is usually `$HOME/.cargo/git/db/`
///
/// The resulting paths are children of this directory in the format
/// [`{last_url_segment || "_empty"}-{hash(url)}`]
/// (https://github.com/rust-lang/cargo/blob/74f2b400d2be43da798f99f94957d359bc223988/src/cargo/sources/git/source.rs#L62-L73)
pub fn find_git_db_repo(git_db_dir: &Path, url: &str) -> Option<PathBuf> {
    let path = git_db_dir.join(format!("{}-{}",
                                       match Url::parse(url)
                                           .ok()?
                                           .path_segments()
                                           .and_then(|segs| segs.rev().next())
                                           .unwrap_or("") {
                                           "" => "_empty",
                                           url => url,
                                       },
                                       cargo_hash(url)));

    if path.is_dir() { Some(path) } else { None }
}


/// The short filesystem name for the repository, as used by `cargo`
///
/// Must be equivalent to
/// https://github.com/rust-lang/cargo/blob/74f2b400d2be43da798f99f94957d359bc223988/src/cargo/sources/registry/mod.rs#L387-L402
/// and
/// https://github.com/rust-lang/cargo/blob/74f2b400d2be43da798f99f94957d359bc223988/src/cargo/util/hex.rs
///
/// For main repository it's `github.com-1ecc6299db9ec823`
pub fn registry_shortname(url: &str) -> String {
    struct RegistryHash<'u>(&'u str);
    impl<'u> Hash for RegistryHash<'u> {
        fn hash<S: Hasher>(&self, hasher: &mut S) {
            SourceKind::Registry.hash(hasher);
            self.0.hash(hasher);
        }
    }

    format!("{}-{}",
            Url::parse(url).map_err(|e| format!("{} not an URL: {}", url, e)).unwrap().host_str().unwrap_or(""),
            cargo_hash(RegistryHash(url)))
}

/// Stolen from and equivalent to `short_hash()` from
/// https://github.com/rust-lang/cargo/blob/74f2b400d2be43da798f99f94957d359bc223988/src/cargo/util/hex.rs
#[allow(deprecated)]
pub fn cargo_hash<T: Hash>(whom: T) -> String {
    use std::hash::SipHasher;

    let mut hasher = SipHasher::new_with_keys(0, 0);
    whom.hash(&mut hasher);
    let hash = hasher.finish();
    hex::encode(&[(hash >> 0) as u8,
                  (hash >> 8) as u8,
                  (hash >> 16) as u8,
                  (hash >> 24) as u8,
                  (hash >> 32) as u8,
                  (hash >> 40) as u8,
                  (hash >> 48) as u8,
                  (hash >> 56) as u8])
}

/// These two are stolen verbatim from
/// https://github.com/rust-lang/cargo/blob/74f2b400d2be43da798f99f94957d359bc223988/src/cargo/core/source/source_id.rs#L48-L73
/// in order to match our hash with
/// https://github.com/rust-lang/cargo/blob/74f2b400d2be43da798f99f94957d359bc223988/src/cargo/core/source/source_id.rs#L510
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(unused)]
enum SourceKind {
    Git(GitReference),
    Path,
    Registry,
    LocalRegistry,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(unused)]
enum GitReference {
    Tag(String),
    Branch(String),
    Rev(String),
}


trait SemverExt {
    fn is_prerelease(&self) -> bool;
}
impl SemverExt for Semver {
    fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}
