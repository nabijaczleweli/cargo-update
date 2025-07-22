//! Main functions doing actual work.
//!
//! Use `installed_registry_packages()` to list the installed packages,
//! then use `intersect_packages()` to confirm which ones should be updated,
//! poll the packages' latest versions by calling `RegistryPackage::pull_version()` on them,
//! continue with doing whatever you wish.


use git2::{self, ErrorCode as GitErrorCode, Config as GitConfig, Error as GitError, Cred as GitCred, RemoteCallbacks, CredentialType, FetchOptions,
           ProxyOptions, Repository, Tree, Oid};
use curl::easy::{WriteError as CurlWriteError, Handler as CurlHandler, SslOpt as CurlSslOpt, Easy2 as CurlEasy, List as CurlList};
use semver::{VersionReq as SemverReq, Version as Semver};
use std::io::{self, ErrorKind as IoErrorKind, Write};
use std::collections::{BTreeMap, BTreeSet};
use curl::multi::Multi as CurlMulti;
use std::process::{Command, Stdio};
use std::{cmp, env, mem, str};
use std::ffi::{OsString, OsStr};
use std::path::{PathBuf, Path};
use json_deserializer as json;
use std::hash::{Hasher, Hash};
use std::iter::FromIterator;
use std::fs::{self, File};
use std::time::Duration;
use std::borrow::Cow;
use std::sync::Mutex;
use url::Url;
use toml;
use hex;

mod config;

pub use self::config::*;


// cargo-audit 0.17.5 (registry+https://github.com/rust-lang/crates.io-index)
// cargo-audit 0.17.5 (sparse+https://index.crates.io/)
// -> (name, version, registry)
//    ("cargo-audit", "0.17.5", "https://github.com/rust-lang/crates.io-index")
//    ("cargo-audit", "0.17.5", "https://index.crates.io/")
fn parse_registry_package_ident(ident: &str) -> Option<(&str, &str, &str)> {
    let mut idx = ident.splitn(3, ' ');
    let (name, version, mut reg) = (idx.next()?, idx.next()?, idx.next()?);
    reg = reg.strip_prefix('(')?.strip_suffix(')')?;
    Some((name, version, reg.strip_prefix("registry+").or_else(|| reg.strip_prefix("sparse+"))?))
}
// alacritty 0.1.0 (git+https://github.com/jwilm/alacritty#eb231b3e70b87875df4bdd1974d5e94704024d70)
// chattium-oxide-client 0.1.0
// (git+https://github.com/nabijaczleweli/chattium-oxide-client?branch=master#108a7b94f0e0dcb2a875f70fc0459d5a682df14c)
// -> (name, url, sha)
//    ("alacritty", "https://github.com/jwilm/alacritty", "eb231b3e70b87875df4bdd1974d5e94704024d70")
// ("chattium-oxide-client", "https://github.com/nabijaczleweli/chattium-oxide-client?branch=master",
//                           "108a7b94f0e0dcb2a875f70fc0459d5a682df14c")
fn parse_git_package_ident(ident: &str) -> Option<(&str, &str, &str)> {
    let mut idx = ident.splitn(3, ' ');
    let (name, _, blob) = (idx.next()?, idx.next()?, idx.next()?);
    let (url, sha) = blob.strip_prefix("(git+")?.strip_suffix(')')?.split_once('#')?;
    if sha.len() != 40 {
        return None;
    }
    Some((name, url, sha))
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
    /// The main repository is `https://github.com/rust-lang/crates.io-index`, or `sparse+https://index.crates.io/`.
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
///                newest_id: Err(git2::Error::from_str("")),
///                executables: vec!["alacritty".to_string()],
///            });
///
/// # /*
/// package.pull_version(&registry_tree, &registry);
/// # */
/// # package.newest_id = git2::Oid::from_str("5f7885749c4d7e48869b1fc0be4d430601cdbbfa");
/// assert!(package.newest_id.is_ok());
/// # }
/// ```
#[derive(Debug, PartialEq)]
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
    pub newest_id: Result<Oid, GitError>,
    /// Executables currently installed for this package.
    pub executables: Vec<String>,
}
impl Hash for GitRepoPackage {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.url.hash(state);
        self.branch.hash(state);
        self.id.hash(state);
        match &self.newest_id {
            Ok(nid) => nid.hash(state),
            Err(err) => {
                err.raw_code().hash(state);
                err.raw_class().hash(state);
                err.message().hash(state);
            }
        }
        self.executables.hash(state);
    }
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
        parse_registry_package_ident(what).map(|(name, version, registry)| {
            RegistryPackage {
                name: name.to_string(),
                registry: registry.to_string(),
                version: Some(Semver::parse(version).unwrap()),
                newest_version: None,
                alternative_version: None,
                max_version: None,
                executables: executables,
            }
        })
    }

    fn want_to_install_prerelease(&self, version_to_install: &Semver, install_prereleases: Option<bool>) -> bool {
        if install_prereleases.unwrap_or(false) {
            return true;
        }

        // otherwise only want to install prerelease if the current version is a prerelease with the same maj.min.patch
        match self.version.as_ref() {
            Some(cur) => {
                cur.is_prerelease() && cur.major == version_to_install.major && cur.minor == version_to_install.minor && cur.patch == version_to_install.patch
            }
            None => false,
        }
    }

    /// Read the version list for this crate off the specified repository tree and set the latest and alternative versions.
    pub fn pull_version(&mut self, registry: &RegistryTree, registry_parent: &Registry, install_prereleases: Option<bool>) {
        let mut vers_git;
        let vers = match (registry, registry_parent) {
            (RegistryTree::Git(registry), Registry::Git(registry_parent)) => {
                vers_git = find_package_data(&self.name, registry, registry_parent)
                    .ok_or_else(|| format!("package {} not found", self.name))
                    .and_then(|pd| crate_versions(&pd).map_err(|e| format!("package {}: {}", self.name, e)))
                    .unwrap();
                vers_git.sort();
                &vers_git
            }
            (RegistryTree::Sparse, Registry::Sparse(registry_parent)) => &registry_parent[&self.name],
            _ => unreachable!(),
        };

        self.newest_version = None;
        self.alternative_version = None;

        let mut vers = vers.iter().rev();
        if let Some(newest) = vers.next() {
            self.newest_version = Some(newest.clone());

            if self.newest_version.as_ref().unwrap().is_prerelease() &&
               !self.want_to_install_prerelease(self.newest_version.as_ref().unwrap(), install_prereleases) {
                if let Some(newest_nonpre) = vers.find(|v| !v.is_prerelease()) {
                    mem::swap(&mut self.alternative_version, &mut self.newest_version);
                    self.newest_version = Some(newest_nonpre.clone());
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
                (!upd_v.is_prerelease() || self.want_to_install_prerelease(upd_v, install_prereleases)) &&
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
    ///                newest_id: Err(git2::Error::from_str("")),
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
    ///                newest_id: Err(git2::Error::from_str("")),
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
        parse_git_package_ident(what).map(|(name, url, sha)| {
            let mut url = Url::parse(url).unwrap();
            let branch = url.query_pairs().find(|&(ref name, _)| name == "branch").map(|(_, value)| value.to_string());
            url.set_query(None);
            GitRepoPackage {
                name: name.to_string(),
                url: url.into(),
                branch: branch,
                id: Oid::from_str(sha).unwrap(),
                newest_id: Err(GitError::from_str("")),
                executables: executables,
            }
        })
    }

    /// Clone the repo and check what the latest commit's hash is.
    pub fn pull_version<Pt: AsRef<Path>, Pg: AsRef<Path>>(&mut self, temp_dir: Pt, git_db_dir: Pg, http_proxy: Option<&str>, fork_git: bool) {
        self.pull_version_impl(temp_dir.as_ref(), git_db_dir.as_ref(), http_proxy, fork_git)
    }

    fn pull_version_impl(&mut self, temp_dir: &Path, git_db_dir: &Path, http_proxy: Option<&str>, fork_git: bool) {
        let clone_dir = find_git_db_repo(git_db_dir, &self.url).unwrap_or_else(|| temp_dir.join(&self.name));
        if !clone_dir.exists() {
            self.newest_id = if fork_git {
                Command::new(env::var_os("GIT").as_ref().map(OsString::as_os_str).unwrap_or(OsStr::new("git")))
                    .args(&["ls-remote", "--", &self.url, self.branch.as_ref().map(String::as_str).unwrap_or("HEAD")])
                    .arg(&clone_dir)
                    .stderr(Stdio::inherit())
                    .output()
                    .ok()
                    .filter(|s| s.status.success())
                    .map(|s| s.stdout)
                    .and_then(|o| String::from_utf8(o).ok())
                    .and_then(|o| o.split('\t').next().and_then(|o| Oid::from_str(o).ok()))
                    .ok_or(GitError::from_str(""))
            } else {
                with_authentication(&self.url, |creds| {
                    git2::Remote::create_detached(self.url.clone()).and_then(|mut r| {
                        let mut cb = RemoteCallbacks::new();
                        cb.credentials(|a, b, c| creds(a, b, c));
                        r.connect_auth(git2::Direction::Fetch,
                                          Some(cb),
                                          http_proxy.map(|http_proxy| proxy_options_from_proxy_url(&self.url, http_proxy)))
                            .and_then(|rc| {
                                rc.list()?
                                    .into_iter()
                                    .find(|rh| match self.branch.as_ref() {
                                        Some(b) => {
                                            if rh.name().starts_with("refs/heads/") {
                                                rh.name()["refs/heads/".len()..] == b[..]
                                            } else if rh.name().starts_with("refs/tags/") {
                                                rh.name()["refs/tags/".len()..] == b[..]
                                            } else {
                                                false
                                            }
                                        }
                                        None => rh.name() == "HEAD",
                                    })
                                    .map(|rh| rh.oid())
                                    .ok_or(git2::Error::from_str(""))
                            })
                    })
                })
            };
            if self.newest_id.is_ok() {
                return;
            }
        }

        let repo = self.pull_version_repo(&clone_dir, http_proxy, fork_git);

        self.newest_id = repo.and_then(|r| r.head().and_then(|h| h.target().ok_or_else(|| GitError::from_str("HEAD not a direct reference"))));
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
            let _ = fs::remove_dir_all(&clone_dir).or_else(|_| fs::remove_file(&clone_dir));

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
    ///             newest_id: git2::Oid::from_str("5f7885749c4d7e48869b1fc0be4d430601cdbbfa"),
    ///             executables: vec!["alacritty".to_string()],
    ///         }.needs_update());
    /// assert!(!GitRepoPackage {
    ///             name: "alacritty".to_string(),
    ///             url: "https://github.com/jwilm/alacritty".to_string(),
    ///             branch: None,
    ///             id: git2::Oid::from_str("5f7885749c4d7e48869b1fc0be4d430601cdbbfa").unwrap(),
    ///             newest_id: git2::Oid::from_str("5f7885749c4d7e48869b1fc0be4d430601cdbbfa"),
    ///             executables: vec!["alacritty".to_string()],
    ///         }.needs_update());
    /// # }
    /// ```
    pub fn needs_update(&self) -> bool {
        self.newest_id.is_ok() && self.id != *self.newest_id.as_ref().unwrap()
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
    /// https://blog.rust-lang.org/2023/03/09/Rust-1.68.0.html#cargos-sparse-protocol
    /// https://doc.rust-lang.org/stable/cargo/reference/registry-index.html#sparse-protocol
    pub registries_crates_io_protocol_sparse: bool,
    pub http: HttpCargoConfig,
    pub sparse_registries: SparseRegistryConfig,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct HttpCargoConfig {
    pub cainfo: Option<PathBuf>,
    pub check_revoke: bool,
}

/// https://github.com/nabijaczleweli/cargo-update/issues/300
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct SparseRegistryConfig {
    pub global_credential_providers: Vec<SparseRegistryAuthProvider>,
    pub crates_io_credential_provider: Option<[SparseRegistryAuthProvider; 1]>,
    pub crates_io_token_env: Option<String>,
    pub crates_io_token: Option<String>,
    pub registry_tokens_env: BTreeMap<CargoConfigEnvironmentNormalisedString, String>,
    pub registry_tokens: BTreeMap<String, String>,
    pub credential_aliases: BTreeMap<CargoConfigEnvironmentNormalisedString, Vec<String>>,
}

impl SparseRegistryConfig {
    pub fn credential_provider(&self, v: toml::Value) -> Option<SparseRegistryAuthProvider> {
        SparseRegistryConfig::credential_provider_impl(&self.credential_aliases, v)
    }

    fn credential_provider_impl(credential_aliases: &BTreeMap<CargoConfigEnvironmentNormalisedString, Vec<String>>, v: toml::Value)
                                -> Option<SparseRegistryAuthProvider> {
        match v {
            toml::Value::String(s) => Some(CargoConfig::string_provider(s, &credential_aliases)),
            toml::Value::Array(a) => Some(SparseRegistryAuthProvider::Provider(CargoConfig::string_array(a))),
            _ => None,
        }
    }
}

/// https://doc.rust-lang.org/cargo/reference/registry-authentication.html
///
/// Not implemented: `cargo:macos-keychain`
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum SparseRegistryAuthProvider {
    /// The default; does not read `CARGO_REGISTRY_TOKEN` or `CARGO_REGISTRIES_{}_TOKEN` environment variables.
    TokenNoEnvironment,
    /// `cargo:token`
    Token,
    /// `cargo:wincred` (not implemented)
    Wincred,
    /// `cargo:macos-keychain` (not implemented)
    MacosKeychain,
    /// `cargo:libsecret` (not implemented)
    Libsecret,
    /// `cargo:token-from-stdout prog arg arg`
    TokenFromStdout(Vec<String>),
    /// Not `cargo:`-prefixed (not implemented)
    ///
    /// https://doc.rust-lang.org/cargo/reference/credential-provider-protocol.html
    Provider(Vec<String>),
}

impl SparseRegistryAuthProvider {
    /// Parses a `cargo:token-from-stdout whatever`-style entry
    pub fn from_config(s: &str) -> SparseRegistryAuthProvider {
        let mut toks = s.split(' ').peekable();
        match toks.peek().unwrap_or(&"") {
            &"cargo:token" => SparseRegistryAuthProvider::Token,
            &"cargo:wincred" => SparseRegistryAuthProvider::Wincred,
            &"cargo:macos-keychain" => SparseRegistryAuthProvider::MacosKeychain,
            &"cargo:libsecret" => SparseRegistryAuthProvider::Libsecret,
            &"cargo:token-from-stdout" => SparseRegistryAuthProvider::TokenFromStdout(toks.skip(1).map(String::from).collect()),
            _ => SparseRegistryAuthProvider::Provider(toks.map(String::from).collect()),
        }
    }
}

/// https://doc.rust-lang.org/cargo/reference/config.html#environment-variables
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct CargoConfigEnvironmentNormalisedString(pub String);
impl CargoConfigEnvironmentNormalisedString {
    /// `tr a-z.- A-Z__`
    pub fn normalise(mut s: String) -> CargoConfigEnvironmentNormalisedString {
        s.make_ascii_uppercase();
        while let Some(i) = s.find(['.', '-']) {
            s.replace_range(i..i + 1, "_");
        }
        CargoConfigEnvironmentNormalisedString(s)
    }
}

impl CargoConfig {
    pub fn load(crates_file: &Path) -> CargoConfig {
        let mut cfg = fs::read_to_string(crates_file.with_file_name("config"))
            .or_else(|_| fs::read_to_string(crates_file.with_file_name("config.toml")))
            .ok()
            .and_then(|s| s.parse::<toml::Value>().ok());
        let mut creds = fs::read_to_string(crates_file.with_file_name("credentials"))
            .or_else(|_| fs::read_to_string(crates_file.with_file_name("credentials.toml")))
            .ok()
            .and_then(|s| s.parse::<toml::Value>().ok());

        let credential_aliases = None.or_else(|| match cfg.as_mut()?.as_table_mut()?.remove("credential-alias")? {
                toml::Value::Table(t) => Some(t),
                _ => None,
            })
            .unwrap_or_default()
            .into_iter()
            .flat_map(|(k, v)| {
                match v {
                        toml::Value::String(s) => Some(s.split(' ').map(String::from).collect()),
                        toml::Value::Array(a) => Some(CargoConfig::string_array(a)),
                        _ => None,
                    }
                    .map(|v| (CargoConfigEnvironmentNormalisedString::normalise(k), v))
            })
            .chain(env::vars_os()
                .map(|(k, v)| (k.into_encoded_bytes(), v))
                .filter(|(k, _)| k.starts_with(b"CARGO_CREDENTIAL_ALIAS_"))
                .filter(|(k, _)| k["CARGO_CREDENTIAL_ALIAS_".len()..].iter().all(|&b| !(b.is_ascii_lowercase() || b == b'.' || b == b'-')))
                .flat_map(|(mut k, v)| {
                    let k = String::from_utf8(k.drain("CARGO_CREDENTIAL_ALIAS_".len()..).collect()).ok()?;
                    let v = v.into_string().ok()?;
                    Some((CargoConfigEnvironmentNormalisedString(k), v.split(' ').map(String::from).collect()))
                }))
            .collect();

        CargoConfig {
            net_git_fetch_with_cli: env::var("CARGO_NET_GIT_FETCH_WITH_CLI")
                .ok()
                .and_then(|e| if e.is_empty() {
                    Some(toml::Value::String(String::new()))
                } else {
                    e.parse::<toml::Value>().ok()
                })
                .or_else(|| {
                    cfg.as_mut()?
                        .as_table_mut()?
                        .get_mut("net")?
                        .as_table_mut()?
                        .remove("git-fetch-with-cli")
                })
                .map(CargoConfig::truthy)
                .unwrap_or(false),
            registries_crates_io_protocol_sparse: env::var("CARGO_REGISTRIES_CRATES_IO_PROTOCOL")
                .map(|s| s == "sparse")
                .ok()
                .or_else(|| {
                    Some(cfg.as_mut()?
                        .as_table_mut()?
                        .get_mut("registries")?
                        .as_table_mut()?
                        .get_mut("crates-io")?
                        .as_table_mut()?
                        .remove("protocol")?
                        .as_str()? == "sparse")
                })
                // // Horrifically expensive (82-93ms end-to-end) and largely unnecessary
                // .or_else(|| {
                //     let mut l = String::new();
                //     // let before = std::time::Instant::now();
                //     BufReader::new(Command::new(cargo).arg("version").stdout(Stdio::piped()).spawn().ok()?.stdout?).read_line(&mut l).ok()?;
                //     // let after = std::time::Instant::now();
                //
                //     // cargo 1.63.0 (fd9c4297c 2022-07-01)
                //     Some(Semver::parse(l.split_whitespace().nth(1)?).ok()? >= Semver::new(1, 70, 0))
                // })
                // .unwrap_or(false),
                .unwrap_or(true),
            http: HttpCargoConfig {
                cainfo: env::var_os("CARGO_HTTP_CAINFO")
                    .map(PathBuf::from)
                    .or_else(|| {
                        CargoConfig::string(cfg.as_mut()?
                                .as_table_mut()?
                                .get_mut("http")?
                                .as_table_mut()?
                                .remove("cainfo")?)
                            .map(PathBuf::from)
                    }),
                check_revoke: env::var("CARGO_HTTP_CHECK_REVOKE")
                    .ok()
                    .map(toml::Value::String)
                    .or_else(|| {
                        cfg.as_mut()?
                            .as_table_mut()?
                            .get_mut("http")?
                            .as_table_mut()?
                            .remove("check-revoke")
                    })
                    .map(CargoConfig::truthy)
                    .unwrap_or(cfg!(target_os = "windows")),
            },
            sparse_registries: SparseRegistryConfig {
                // Supposedly this is CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS but they don't specify how they serialise arrays so
                global_credential_providers: None.or_else(|| {
                        CargoConfig::string_array_v(cfg.as_mut()?
                            .as_table_mut()?
                            .get_mut("registry")?
                            .as_table_mut()?
                            .remove("global-credential-providers")?)
                    })
                    .map(|a| a.into_iter().map(|s| CargoConfig::string_provider(s, &credential_aliases)).collect())
                    .unwrap_or_else(|| vec![SparseRegistryAuthProvider::TokenNoEnvironment]),
                crates_io_credential_provider: env::var("CARGO_REGISTRY_CREDENTIAL_PROVIDER")
                    .ok()
                    .map(toml::Value::String)
                    .or_else(|| {
                        cfg.as_mut()?
                            .as_table_mut()?
                            .get_mut("registry")?
                            .as_table_mut()?
                            .remove("credential-provider")
                    })
                    .and_then(|v| SparseRegistryConfig::credential_provider_impl(&credential_aliases, v).map(|v| [v])),
                crates_io_token_env: env::var("CARGO_REGISTRY_TOKEN").ok(),
                crates_io_token: None.or_else(|| {
                        CargoConfig::string(creds.as_mut()?
                            .as_table_mut()?
                            .get_mut("registry")?
                            .as_table_mut()?
                            .remove("token")?)
                    })
                    .or_else(|| {
                        CargoConfig::string(cfg.as_mut()?
                            .as_table_mut()?
                            .get_mut("registry")?
                            .as_table_mut()?
                            .remove("token")?)
                    }),
                registry_tokens_env: env::vars_os()
                    .map(|(k, v)| (k.into_encoded_bytes(), v))
                    .filter(|(k, _)| k.starts_with(b"CARGO_REGISTRIES_") && k.ends_with(b"_TOKEN"))
                    .filter(|(k, _)| {
                        k["CARGO_REGISTRIES_".len()..k.len() - b"_TOKEN".len()].iter().all(|&b| !(b.is_ascii_lowercase() || b == b'.' || b == b'-'))
                    })
                    .flat_map(|(mut k, v)| {
                        let k = String::from_utf8(k.drain("CARGO_REGISTRIES_".len()..k.len() - b"_TOKEN".len()).collect()).ok()?;
                        Some((CargoConfigEnvironmentNormalisedString(k), v.into_string().ok()?))
                    })
                    .collect(),
                registry_tokens: cfg.as_mut()
                    .into_iter()
                    .chain(creds.as_mut())
                    .flat_map(|c| {
                        c.as_table_mut()?
                            .get_mut("registries")?
                            .as_table_mut()
                    })
                    .flat_map(|r| r.into_iter().flat_map(|(name, v)| Some((name.clone(), CargoConfig::string(v.as_table_mut()?.remove("token")?)?))))
                    .collect(),
                credential_aliases: credential_aliases,
            },
        }
    }

    fn truthy(v: toml::Value) -> bool {
        match v {
            toml::Value::String(ref s) if s == "" => false,
            toml::Value::Float(0.) => false,
            toml::Value::Integer(0) |
            toml::Value::Boolean(false) => false,
            _ => true,
        }
    }

    fn string(v: toml::Value) -> Option<String> {
        match v {
            toml::Value::String(s) => Some(s),
            _ => None,
        }
    }

    fn string_array(a: Vec<toml::Value>) -> Vec<String> {
        a.into_iter().flat_map(CargoConfig::string).collect()
    }

    fn string_array_v(v: toml::Value) -> Option<Vec<String>> {
        match v {
            toml::Value::Array(s) => Some(CargoConfig::string_array(s)),
            _ => None,
        }
    }

    fn string_provider(s: String, credential_aliases: &BTreeMap<CargoConfigEnvironmentNormalisedString, Vec<String>>) -> SparseRegistryAuthProvider {
        match credential_aliases.get(&CargoConfigEnvironmentNormalisedString::normalise(s.clone())) {
            Some(av) => SparseRegistryAuthProvider::Provider(av.clone()),
            None => SparseRegistryAuthProvider::from_config(&s),
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
    crates_file_in_impl(cargo_dir, BTreeSet::new())
}
fn crates_file_in_impl<'cd>(cargo_dir: &'cd Path, mut seen: BTreeSet<&'cd Path>) -> PathBuf {
    if !seen.insert(cargo_dir) {
        panic!("Cargo config install.root loop at {:?} (saw {:?})", cargo_dir.display(), seen);
    }

    let mut config_file = cargo_dir.join("config");
    let mut config_f = File::open(&config_file);
    if config_f.is_err() {
        config_file.set_file_name("config.toml");
        config_f = File::open(&config_file);
    }
    if let Ok(mut config_f) = config_f {
        if let Some(idir) = toml::from_str::<toml::Value>(&io::read_to_string(&mut config_f).unwrap())
            .unwrap()
            .get("install")
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("root"))
            .and_then(|t| t.as_str()) {
            return crates_file_in_impl(Path::new(idir), seen);
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
    if let Ok(crates_file) = fs::read_to_string(crates_file) {
        let mut res = Vec::<RegistryPackage>::new();
        for pkg in match toml::from_str::<toml::Value>(&crates_file).unwrap().get("v1") {
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
    if let Ok(crates_file) = fs::read_to_string(crates_file) {
        let mut res = Vec::<GitRepoPackage>::new();
        for pkg in match toml::from_str::<toml::Value>(&crates_file).unwrap().get("v1") {
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
            .filter(|p| allow_installs && !installed.iter().any(|i| i.name == p.0) && !installed_git.iter().any(|i| i.name == p.0))
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
/// # use std::fs;
/// # let desc_path = "test-data/checksums-versions.json";
/// # let package = "checksums";
/// let versions = crate_versions(&fs::read(desc_path).unwrap()).expect(package);
///
/// println!("Released versions of checksums:");
/// for ver in &versions {
///     println!("  {}", ver);
/// }
/// ```
pub fn crate_versions(buf: &[u8]) -> Result<Vec<Semver>, Cow<'static, str>> {
    buf.split(|&b| b == b'\n').filter(|l| !l.is_empty()).try_fold(vec![], |mut acc, p| match json::parse(p).map_err(|e| e.to_string())? {
        json::Value::Object(o) => {
            if !matches!(o.get("yanked"), Some(&json::Value::Bool(true))) {
                match o.get("vers").ok_or("no \"vers\" key")? {
                    json::Value::String(ref v) => acc.push(Semver::parse(&v).map_err(|e| e.to_string())?),
                    _ => Err("\"vers\" not string")?,
                }
            }
            Ok(acc)
        }
        _ => Err(Cow::from("line not object")),
    })
}

/// Get the location of the registry index corresponding ot the given URL; if not present – make it and its parents.
///
/// As odd as it may be, this [can happen (if rarely) and is a supported
/// configuration](https://github.com/nabijaczleweli/cargo-update/issues/150).
///
/// Sparse registries do nothing and return a meaningless value.
///
/// # Examples
///
/// ```
/// # #[cfg(all(target_pointer_width="64", target_endian="little"))] // https://github.com/nabijaczleweli/cargo-update/issues/235
/// # {
/// # use cargo_update::ops::assert_index_path;
/// # use std::env::temp_dir;
/// # use std::path::Path;
/// # let cargo_dir = temp_dir().join("cargo_update-doctest").join("assert_index_path-0");
/// # let idx_dir = cargo_dir.join("registry").join("index").join("github.com-1ecc6299db9ec823");
/// let index = assert_index_path(&cargo_dir, "https://github.com/rust-lang/crates.io-index", false).unwrap();
///
/// // Use find_package_data() to look for packages
/// # assert_eq!(index, idx_dir);
/// # assert_eq!(assert_index_path(&cargo_dir, "https://index.crates.io/", true).unwrap(), Path::new("/ENOENT"));
/// # }
/// ```
pub fn assert_index_path(cargo_dir: &Path, registry_url: &str, sparse: bool) -> Result<PathBuf, Cow<'static, str>> {
    if sparse {
        return Ok(PathBuf::from("/ENOENT"));
    }

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

/// Opens or initialises a git repository at `registry`, or returns a blank sparse registry.
///
/// Error type distinguishes init error from open error.
pub fn open_index_repository(registry: &Path, sparse: bool) -> Result<Registry, (bool, GitError)> {
    match sparse {
        false => {
            Repository::open(&registry).map(Registry::Git).or_else(|e| if e.code() == GitErrorCode::NotFound {
                Repository::init(&registry).map(Registry::Git).map_err(|e| (true, e))
            } else {
                Err((false, e))
            })
        }
        true => Ok(Registry::Sparse(BTreeMap::new())),
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct SparseRegistryAuthProviderBundle<'sr>(pub Cow<'sr, [SparseRegistryAuthProvider]>,
                                                 pub &'sr OsStr,
                                                 pub &'sr str,
                                                 pub &'sr str,
                                                 pub Option<&'sr str>,
                                                 pub Option<&'sr str>);
impl<'sr> SparseRegistryAuthProviderBundle<'sr> {
    pub fn try(&self) -> Option<Cow<'sr, str>> {
        let (repo_name, install_cargo, repo_url, token_env, token) = (self.1, self.2, self.3, self.4, self.5);
        self.0
            .iter()
            .rev()
            .find_map(|p| match p {
                SparseRegistryAuthProvider::TokenNoEnvironment => token.map(Cow::from),
                SparseRegistryAuthProvider::Token => token_env.or(token).map(Cow::from),
                SparseRegistryAuthProvider::Wincred => None, // TODO
                SparseRegistryAuthProvider::MacosKeychain => None, // TODO
                SparseRegistryAuthProvider::Libsecret => None, // TODO
                SparseRegistryAuthProvider::TokenFromStdout(args) => {
                    Command::new(&args[0])
                        .args(&args[1..])
                        .env("CARGO", install_cargo)
                        .env("CARGO_REGISTRY_INDEX_URL", repo_url)
                        .env("CARGO_REGISTRY_NAME_OPT", repo_name)
                        .stdin(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .output()
                        .ok()
                        .filter(|o| o.status.success())
                        .map(|o| o.stdout)
                        .and_then(|o| String::from_utf8(o).ok())
                        .map(|mut o| {
                            o.replace_range(o.rfind(|c| c != '\n').unwrap_or(o.len()) + 1..o.len(), "");
                            o.replace_range(0..o.find(|c| c != '\n').unwrap_or(0), "");
                            o.into()
                        })
                }
                SparseRegistryAuthProvider::Provider(_) => None, // TODO
            })
    }
}

/// Collect everything needed to get an authentication token for the given registry.
pub fn auth_providers<'sr>(crates_file: &Path, install_cargo: Option<&'sr OsStr>, sparse_registries: &'sr SparseRegistryConfig, sparse: bool,
                           repo_name: &'sr str, repo_url: &'sr str)
                           -> SparseRegistryAuthProviderBundle<'sr> {
    let cargo = install_cargo.unwrap_or(OsStr::new("cargo"));
    if !sparse {
        return SparseRegistryAuthProviderBundle(vec![].into(), cargo, "!sparse", "!sparse", None, None);
    }

    if repo_name == "crates-io" {
        let ret = match sparse_registries.crates_io_credential_provider.as_ref() {
            Some(prov) => prov[..].into(),
            None => sparse_registries.global_credential_providers[..].into(),
        };
        return SparseRegistryAuthProviderBundle(ret,
                                                cargo,
                                                repo_name,
                                                repo_url,
                                                sparse_registries.crates_io_token_env.as_deref(),
                                                sparse_registries.crates_io_token.as_deref());
    }

    // Supposedly this is
    //   format!("CARGO_REGISTRIES_{}_CREDENTIAL_PROVIDER",
    //           CargoConfigEnvironmentNormalisedString::normalise(repo_name.to_string()).0)
    // but they don't specify how they serialise arrays so
    let ret: Cow<'sr, [SparseRegistryAuthProvider]> = match fs::read_to_string(crates_file.with_file_name("config"))
        .or_else(|_| fs::read_to_string(crates_file.with_file_name("config.toml")))
        .ok()
        .and_then(|s| s.parse::<toml::Value>().ok())
        .and_then(|mut c| {
            sparse_registries.credential_provider(c.as_table_mut()?
                .remove("registries")?
                .as_table_mut()?
                .remove(repo_name)?
                .as_table_mut()?
                .remove("credential-provider")?)
        }) {
        Some(prov) => vec![prov].into(),
        None => sparse_registries.global_credential_providers[..].into(),
    };
    let token_env = if ret.contains(&SparseRegistryAuthProvider::Token) {
        sparse_registries.registry_tokens_env.get(&CargoConfigEnvironmentNormalisedString::normalise(repo_name.to_string())).map(String::as_str)
    } else {
        None
    };
    SparseRegistryAuthProviderBundle(ret,
                                     cargo,
                                     repo_name,
                                     repo_url,
                                     token_env,
                                     sparse_registries.registry_tokens.get(repo_name).map(String::as_str))
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
/// https://github.com/rust-lang/cargo/blob/1ee1ef0ea7ab47d657ca675e3b1bd2fcd68b5aab/src/cargo/sources/registry/remote.rs#L204<br />
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
///
/// # Sparse indices
///
/// Have a `.cache` under the obvious path, then the usual `ca/rg/cargo-update`, but *the file is different than the standard
/// format*: it starts with a ^A or ^C (I'm assuming these are versions, and if I looked at more files I would also've seen
/// ^C), then Some Binary Data, then the ETag(?), then {NUL, version, NUL, usual JSON blob line} repeats.
///
/// I do not wanna be touching that shit. Just suck off all the files.<br />
/// Shoulda stored the blobs verbatim and used `If-Modified-Since`. Too me.
///
/// Only in this mode is the package list used.
pub fn update_index<W: Write, A: AsRef<str>, I: Iterator<Item = A>>(index_repo: &mut Registry, repo_url: &str, packages: I, http_proxy: Option<&str>,
                                                                    fork_git: bool, http: &HttpCargoConfig, auth_providers: SparseRegistryAuthProviderBundle,
                                                                    out: &mut W)
                                                                    -> Result<(), String> {
    write!(out,
           "    {} registry '{}'{}",
           ["Updating", "Polling"][matches!(index_repo, Registry::Sparse(_)) as usize],
           repo_url,
           ["\n", ""][matches!(index_repo, Registry::Sparse(_)) as usize]).and_then(|_| out.flush())
        .map_err(|e| format!("failed to write updating message: {}", e))?;
    match index_repo {
        Registry::Git(index_repo) => {
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
        }
        Registry::Sparse(registry) => {
            let auth = auth_providers.try();

            let mut sucker = CurlMulti::new();
            sucker.pipelining(true, true).map_err(|e| format!("pipelining: {}", e))?;

            let writussy = Mutex::new(&mut *out);
            let mut conns: Vec<_> = Result::from_iter(packages.map(|pkg| {
                let mut conn = CurlEasy::new(SparseHandler(pkg.as_ref().to_string(), vec![], Some(&writussy)));
                conn.url(&split_package_path(pkg.as_ref()).into_iter().fold(repo_url.to_string(), |mut u, s| {
                        if !u.ends_with('/') {
                            u.push('/');
                        }
                        u.push_str(&s);
                        u
                    }))
                    .map_err(|e| format!("url: {}", e))?;
                if let Some(auth) = auth.as_ref() {
                    let mut headers = CurlList::new();
                    headers.append(&format!("Authorization: {}", auth)).map_err(|e| format!("append: {}", e))?;
                    conn.http_headers(headers).map_err(|e| format!("http_headers: {}", e))?;
                }
                if let Some(http_proxy) = http_proxy {
                    conn.proxy(http_proxy).map_err(|e| format!("proxy: {}", e))?;
                }
                conn.pipewait(true).map_err(|e| format!("pipewait: {}", e))?;
                conn.progress(true).map_err(|e| format!("progress: {}", e))?;
                if let Some(cainfo) = http.cainfo.as_ref() {
                    conn.cainfo(cainfo).map_err(|e| format!("cainfo: {}", e))?;
                }
                conn.ssl_options(CurlSslOpt::new().no_revoke(!http.check_revoke)).map_err(|e| format!("ssl_options: {}", e))?;
                sucker.add2(conn).map(|h| (h, Ok(()))).map_err(|e| format!("add2: {}", e))
            }))?;

            while sucker.perform().map_err(|e| format!("perform: {}", e))? > 0 {
                sucker.wait(&mut [], Duration::from_millis(200)).map_err(|e| format!("wait: {}", e))?;
            }

            writussy.lock()
                .map_err(|e| e.to_string())
                .and_then(|mut out| writeln!(out).map_err(|e| e.to_string()))
                .map_err(|e| format!("failed to write post-update newline: {}", e))?;

            sucker.messages(|m| {
                for c in &mut conns {
                    // Yes, a linear search; this is much faster than adding 2+n sets of CURLINFO_PRIVATE calls
                    if let Some(err) = m.result_for2(&c.0) {
                        c.1 = err;
                    }
                }
            });

            for mut c in conns {
                let pkg = mem::take(&mut c.0.get_mut().0);
                if let Err(e) = c.1 {
                    return Err(format!("package {}: {}", pkg, e));
                }
                match c.0.response_code().map_err(|e| format!("response_code: {}", e))? {
                    200 => {
                        let mut resp = crate_versions(&c.0.get_ref().1).map_err(|e| format!("package {}: {}", pkg, e))?;
                        resp.sort();
                        registry.insert(pkg, resp);
                    }
                    rc @ 404 | rc @ 410 | rc @ 451 => return Err(format!("package {} doesn't exist: HTTP {}", pkg, rc)),
                    rc => return Err(format!("package {}: HTTP {}", pkg, rc)),
                }
            }
        }
    }
    writeln!(out).map_err(|e| format!("failed to write post-update newline: {}", e))?;

    Ok(())
}

// Could we theoretically parse the semvers on the fly? Yes. Is it more trouble than it's worth? Also probably yes; there
// doesn't appear to be a good way to bubble errors.
// Same applies to just waiting instead of processing via .messages()
struct SparseHandler<'m, 'w: 'm, W: Write>(String, Vec<u8>, Option<&'m Mutex<&'w mut W>>);

impl<'m, 'w: 'm, W: Write> CurlHandler for SparseHandler<'m, 'w, W> {
    fn write(&mut self, data: &[u8]) -> Result<usize, CurlWriteError> {
        self.1.extend(data);
        Ok(data.len())
    }
    fn progress(&mut self, dltotal: f64, dlnow: f64, _: f64, _: f64) -> bool {
        if dltotal != 0.0 && dltotal == dlnow {
            if let Some(mut out) = self.2.take().and_then(|m| m.lock().ok()) {
                let _ = out.write_all(b".").and_then(|_| out.flush());
            }
        }
        true
    }
}


/// Either an open git repository with a git registry, or a map of (package, sorted versions), populated by
/// [`update_index()`](fn.update_index.html)
pub enum Registry {
    Git(Repository),
    Sparse(BTreeMap<String, Vec<Semver>>),
}

/// A git tree corresponding to the latest revision of a git registry.
pub enum RegistryTree<'a> {
    Git(Tree<'a>),
    Sparse,
}

/// Get `FETCH_HEAD` or `origin/HEAD`, then unwrap it to the tree it points to.
pub fn parse_registry_head(registry_repo: &Registry) -> Result<RegistryTree, GitError> {
    match registry_repo {
        Registry::Git(registry_repo) => {
            registry_repo.revparse_single("FETCH_HEAD")
                .or_else(|_| registry_repo.revparse_single("origin/HEAD"))
                .map(|h| h.as_commit().unwrap().tree().unwrap())
                .map(RegistryTree::Git)
        }
        Registry::Sparse(_) => Ok(RegistryTree::Sparse),
    }
}


fn proxy_options_from_proxy_url<'a>(repo_url: &str, proxy_url: &str) -> ProxyOptions<'a> {
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
}

fn fetch_options_from_proxy_url_and_callbacks<'a>(repo_url: &str, proxy_url: Option<&str>, callbacks: RemoteCallbacks<'a>) -> FetchOptions<'a> {
    let mut ret = FetchOptions::new();
    if let Some(proxy_url) = proxy_url {
        ret.proxy_options(proxy_options_from_proxy_url(repo_url, proxy_url));
    }
    ret.remote_callbacks(callbacks);
    ret
}

/// Get the URL to update index from, whether it's "sparse", and the cargo name for it from the config file parallel to the
/// specified crates file
///
/// First gets the source name corresponding to the given URL, if appropriate,
/// then chases the `source.$SRCNAME.replace-with` chain,
/// then retrieves the URL from `source.$SRCNAME.registry` of the final source.
///
/// Prepopulates with `source.crates-io.registry = "https://github.com/rust-lang/crates.io-index"`,
/// as specified in the book
///
/// If `registries_crates_io_protocol_sparse`, `https://github.com/rust-lang/crates.io-index` is replaced with
/// `sparse+https://index.crates.io/`.
///
/// Consult [#107](https://github.com/nabijaczleweli/cargo-update/issues/107) and
/// the Cargo Book for details: https://doc.rust-lang.org/cargo/reference/source-replacement.html,
/// https://doc.rust-lang.org/cargo/reference/registries.html.
pub fn get_index_url(crates_file: &Path, registry: &str, registries_crates_io_protocol_sparse: bool)
                     -> Result<(Cow<'static, str>, bool, Cow<'static, str>), Cow<'static, str>> {
    let mut config_file = crates_file.with_file_name("config");
    let config = if let Ok(cfg) = fs::read_to_string(&config_file).or_else(|_| {
        config_file.set_file_name("config.toml");
        fs::read_to_string(&config_file)
    }) {
        toml::from_str::<toml::Value>(&cfg).map_err(|e| format!("{} not TOML: {}", config_file.display(), e))?
    } else {
        if registry == "https://github.com/rust-lang/crates.io-index" {
            if registries_crates_io_protocol_sparse {
                return Ok(("https://index.crates.io/".into(), true, "crates-io".into()));
            } else {
                return Ok((registry.to_string().into(), false, "crates-io".into()));
            }
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
    registries.insert("crates-io",
                      Cow::from(if registries_crates_io_protocol_sparse {
                          "sparse+https://index.crates.io/"
                      } else {
                          "https://github.com/rust-lang/crates.io-index"
                      }));
    if cur_source == "https://github.com/rust-lang/crates.io-index" || cur_source == "sparse+https://index.crates.io/" {
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
            if cur_source == url.strip_prefix("sparse+").unwrap_or(url) {
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

    registries.get(&cur_source[..])
        .map(|reg| (reg.strip_prefix("sparse+").unwrap_or(reg).to_string().into(), reg.starts_with("sparse+"), cur_source.to_string().into()))
        .ok_or_else(|| {
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

        Err(GitError::from_str(&msg))
    }
}


/// Split and lower-case `cargo-update` into `[ca, rg, cargo-update]`, `jot` into `[3, j, jot]`, &c.
pub fn split_package_path(cratename: &str) -> Vec<Cow<str>> {
    let mut elems = Vec::new();
    if cratename.len() <= 3 {
        elems.push(cratename.len().to_string().into());
    }
    match cratename.len() {
        0 => panic!("0-length cratename"),
        1 | 2 => {}
        3 => elems.push(lcase(&cratename[0..1])),
        _ => {
            elems.push(lcase(&cratename[0..2]));
            elems.push(lcase(&cratename[2..4]));
        }
    }
    elems.push(lcase(cratename));
    elems
}

fn lcase(s: &str) -> Cow<str> {
    if s.bytes().any(|b| b.is_ascii_uppercase()) {
        s.to_ascii_lowercase().into()
    } else {
        s.into()
    }
}

/// Find package data in the specified cargo git index tree.
pub fn find_package_data<'t>(cratename: &str, registry: &Tree<'t>, registry_parent: &'t Repository) -> Option<Vec<u8>> {
    let elems = split_package_path(cratename);

    let ent = registry.get_name(&elems[0])?;
    let obj = ent.to_object(registry_parent).ok()?;
    let ent = obj.as_tree()?.get_name(&elems[1])?;
    let obj = ent.to_object(registry_parent).ok()?;
    if elems.len() == 3 {
        let ent = obj.as_tree()?.get_name(&elems[2])?;
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
    if let Ok(crates_file) = fs::read_to_string(crates_file) {
        if let Some(proxy) = toml::from_str::<toml::Value>(&crates_file)
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
                                           .and_then(|mut segs| segs.next_back())
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
