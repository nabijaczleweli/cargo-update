use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as FWrite;
use std::io::{Read, Write};
use std::default::Default;
use semver::VersionReq;
use std::borrow::Cow;
use std::path::Path;
use std::fs::File;
use toml;


/// A single operation to be executed upon configuration of a package.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigOperation {
    /// Set the toolchain to use to compile the package.
    SetToolchain(String),
    /// Use the default toolchain to use to compile the package.
    RemoveToolchain,
    /// Whether to compile the package with the default features.
    DefaultFeatures(bool),
    /// Compile the package with the specified feature.
    AddFeature(String),
    /// Remove the feature from the list of features to compile with.
    RemoveFeature(String),
    /// Set debug mode being enabled to the specified value.
    SetDebugMode(bool),
    /// Set allowing to install prereleases to the specified value.
    SetInstallPrereleases(bool),
    /// Set enforcing Cargo.lock to the specified value.
    SetEnforceLock(bool),
    /// Set installing only the pre-set binaries.
    SetRespectBinaries(bool),
    /// Constrain the installed to the specified one.
    SetTargetVersion(VersionReq),
    /// Always install latest package version.
    RemoveTargetVersion,
}


/// Compilation configuration for one crate.
///
/// # Examples
///
/// Reading a configset, adding an entry to it, then writing it back.
///
/// ```
/// # use cargo_update::ops::PackageConfig;
/// # use std::fs::{File, create_dir_all};
/// # use std::env::temp_dir;
/// # let td = temp_dir().join("cargo_update-doctest").join("PackageConfig-0");
/// # create_dir_all(&td).unwrap();
/// # let config_file = td.join(".install_config.toml");
/// # let operations = [];
/// let mut configuration = PackageConfig::read(&config_file).unwrap();
/// configuration.insert("cargo_update".to_string(), PackageConfig::from(&operations));
/// PackageConfig::write(&configuration, &config_file).unwrap();
/// ```
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PackageConfig {
    /// Toolchain to use to compile the package, or `None` for default.
    pub toolchain: Option<String>,
    /// Whether to compile the package with the default features.
    pub default_features: bool,
    /// Features to compile the package with.
    pub features: BTreeSet<String>,
    /// Whether to compile in debug mode.
    pub debug: Option<bool>,
    /// Whether to install pre-release versions.
    pub install_prereleases: Option<bool>,
    /// Whether to enforce Cargo.lock versions.
    pub enforce_lock: Option<bool>,
    /// Whether to install only the pre-configured binaries.
    pub respect_binaries: Option<bool>,
    /// Versions to constrain to.
    pub target_version: Option<VersionReq>,
}


impl PackageConfig {
    /// Create a package config based on the default settings and modified according to the specified operations.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cargo_update;
    /// # extern crate semver;
    /// # fn main() {
    /// # use cargo_update::ops::{ConfigOperation, PackageConfig};
    /// # use std::collections::BTreeSet;
    /// # use semver::VersionReq;
    /// # use std::str::FromStr;
    /// assert_eq!(PackageConfig::from(&[ConfigOperation::SetToolchain("nightly".to_string()),
    ///                                  ConfigOperation::DefaultFeatures(false),
    ///                                  ConfigOperation::AddFeature("rustc-serialize".to_string()),
    ///                                  ConfigOperation::SetDebugMode(true),
    ///                                  ConfigOperation::SetInstallPrereleases(false),
    ///                                  ConfigOperation::SetEnforceLock(true),
    ///                                  ConfigOperation::SetRespectBinaries(true),
    ///                                  ConfigOperation::SetTargetVersion(VersionReq::from_str(">=0.1").unwrap())]),
    ///            PackageConfig {
    ///                toolchain: Some("nightly".to_string()),
    ///                default_features: false,
    ///                features: {
    ///                    let mut feats = BTreeSet::new();
    ///                    feats.insert("rustc-serialize".to_string());
    ///                    feats
    ///                },
    ///                debug: Some(true),
    ///                install_prereleases: Some(false),
    ///                enforce_lock: Some(true),
    ///                respect_binaries: Some(true),
    ///                target_version: Some(VersionReq::from_str(">=0.1").unwrap()),
    ///            });
    /// # }
    /// ```
    pub fn from<'o, O: IntoIterator<Item = &'o ConfigOperation>>(ops: O) -> PackageConfig {
        let mut def = PackageConfig::default();
        def.execute_operations(ops);
        def
    }

    /// Generate cargo arguments from this configuration.
    ///
    /// Executable names are stripped of their trailing `".exe"`, if any.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use cargo_update::ops::PackageConfig;
    /// # use std::collections::BTreeMap;
    /// # use std::process::Command;
    /// # let name = "cargo-update".to_string();
    /// # let mut configuration = BTreeMap::new();
    /// # configuration.insert(name.clone(), PackageConfig::from(&[]));
    /// let cmd = Command::new("cargo").args(configuration.get(&name).unwrap().cargo_args(&["racer"]).iter().map(AsRef::as_ref)).arg(&name)
    /// // Process the command further -- run it, for example.
    /// # .status().unwrap();
    /// # let _ = cmd;
    /// ```
    pub fn cargo_args<S: AsRef<str>, I: IntoIterator<Item = S>>(&self, executables: I) -> Vec<Cow<'static, str>> {
        let mut res = vec![];
        if let Some(ref t) = self.toolchain {
            res.push(format!("+{}", t).into());
        }
        res.push("install".into());
        res.push("-f".into());
        if !self.default_features {
            res.push("--no-default-features".into());
        }
        if !self.features.is_empty() {
            res.push("--features".into());
            let mut a = String::new();
            for f in &self.features {
                write!(a, "{} ", f).unwrap();
            }
            res.push(a.into());
        }
        if let Some(true) = self.enforce_lock {
            res.push("--locked".into());
        }
        if let Some(true) = self.respect_binaries {
            for x in executables {
                let x = x.as_ref();

                res.push("--bin".into());
                res.push(x.strip_suffix(".exe").unwrap_or(x).to_string().into());
            }
        }
        if let Some(true) = self.debug {
            res.push("--debug".into());
        }
        res
    }

    /// Modify `self` according to the specified set of operations.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cargo_update;
    /// # extern crate semver;
    /// # fn main() {
    /// # use cargo_update::ops::{ConfigOperation, PackageConfig};
    /// # use std::collections::BTreeSet;
    /// # use semver::VersionReq;
    /// # use std::str::FromStr;
    /// let mut cfg = PackageConfig {
    ///     toolchain: Some("nightly".to_string()),
    ///     default_features: false,
    ///     features: {
    ///         let mut feats = BTreeSet::new();
    ///         feats.insert("rustc-serialize".to_string());
    ///         feats
    ///     },
    ///     debug: None,
    ///     install_prereleases: None,
    ///     enforce_lock: None,
    ///     respect_binaries: None,
    ///     target_version: Some(VersionReq::from_str(">=0.1").unwrap()),
    /// };
    /// cfg.execute_operations(&[ConfigOperation::RemoveToolchain,
    ///                          ConfigOperation::AddFeature("serde".to_string()),
    ///                          ConfigOperation::RemoveFeature("rustc-serialize".to_string()),
    ///                          ConfigOperation::SetDebugMode(true),
    ///                          ConfigOperation::RemoveTargetVersion]);
    /// assert_eq!(cfg,
    ///            PackageConfig {
    ///                toolchain: None,
    ///                default_features: false,
    ///                features: {
    ///                    let mut feats = BTreeSet::new();
    ///                    feats.insert("serde".to_string());
    ///                    feats
    ///                },
    ///                debug: Some(true),
    ///                install_prereleases: None,
    ///                enforce_lock: None,
    ///                respect_binaries: None,
    ///                target_version: None,
    ///            });
    /// # }
    /// ```
    pub fn execute_operations<'o, O: IntoIterator<Item = &'o ConfigOperation>>(&mut self, ops: O) {
        for op in ops {
            self.execute_operation(op)
        }
    }

    fn execute_operation(&mut self, op: &ConfigOperation) {
        match *op {
            ConfigOperation::SetToolchain(ref tchn) => self.toolchain = Some(tchn.clone()),
            ConfigOperation::RemoveToolchain => self.toolchain = None,
            ConfigOperation::DefaultFeatures(f) => self.default_features = f,
            ConfigOperation::AddFeature(ref feat) => {
                self.features.insert(feat.clone());
            }
            ConfigOperation::RemoveFeature(ref feat) => {
                self.features.remove(feat);
            }
            ConfigOperation::SetDebugMode(d) => self.debug = Some(d),
            ConfigOperation::SetInstallPrereleases(pr) => self.install_prereleases = Some(pr),
            ConfigOperation::SetEnforceLock(el) => self.enforce_lock = Some(el),
            ConfigOperation::SetRespectBinaries(rb) => self.respect_binaries = Some(rb),
            ConfigOperation::SetTargetVersion(ref vr) => self.target_version = Some(vr.clone()),
            ConfigOperation::RemoveTargetVersion => self.target_version = None,
        }
    }

    /// Read a configset from the specified file.
    ///
    /// If the specified file doesn't exist an empty configset is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::collections::{BTreeSet, BTreeMap};
    /// # use cargo_update::ops::PackageConfig;
    /// # use std::fs::{File, create_dir_all};
    /// # use std::env::temp_dir;
    /// # use std::io::Write;
    /// # let td = temp_dir().join("cargo_update-doctest").join("PackageConfig-read-0");
    /// # create_dir_all(&td).unwrap();
    /// # let config_file = td.join(".install_config.toml");
    /// File::create(&config_file).unwrap().write_all(&b"\
    ///    [cargo-update]\n\
    ///    default_features = true\n\
    ///    features = [\"serde\"]\n"[..]);
    /// assert_eq!(PackageConfig::read(&config_file), Ok({
    ///     let mut pkgs = BTreeMap::new();
    ///     pkgs.insert("cargo-update".to_string(), PackageConfig {
    ///         toolchain: None,
    ///         default_features: true,
    ///         features: {
    ///             let mut feats = BTreeSet::new();
    ///             feats.insert("serde".to_string());
    ///             feats
    ///         },
    ///         debug: None,
    ///         install_prereleases: None,
    ///         enforce_lock: None,
    ///         respect_binaries: None,
    ///         target_version: None,
    ///     });
    ///     pkgs
    /// }));
    /// ```
    pub fn read(p: &Path) -> Result<BTreeMap<String, PackageConfig>, i32> {
        if p.exists() {
            let mut buf = String::new();
            File::open(p).map_err(|_| 1)?
                .read_to_string(&mut buf)
                .map_err(|_| 1)?;

            toml::from_str(&buf).map_err(|_| 2)
        } else {
            Ok(BTreeMap::new())
        }
    }

    /// Save a configset to the specified file.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::collections::{BTreeSet, BTreeMap};
    /// # use cargo_update::ops::PackageConfig;
    /// # use std::fs::{File, create_dir_all};
    /// # use std::env::temp_dir;
    /// # use std::io::Read;
    /// # let td = temp_dir().join("cargo_update-doctest").join("PackageConfig-write-0");
    /// # create_dir_all(&td).unwrap();
    /// # let config_file = td.join(".install_config.toml");
    /// PackageConfig::write(&{
    ///     let mut pkgs = BTreeMap::new();
    ///     pkgs.insert("cargo-update".to_string(), PackageConfig {
    ///         toolchain: None,
    ///         default_features: true,
    ///         features: {
    ///             let mut feats = BTreeSet::new();
    ///             feats.insert("serde".to_string());
    ///             feats
    ///         },
    ///         debug: None,
    ///         install_prereleases: None,
    ///         enforce_lock: None,
    ///         respect_binaries: None,
    ///         target_version: None,
    ///     });
    ///     pkgs
    /// }, &config_file).unwrap();
    ///
    /// let mut buf = String::new();
    /// File::open(&config_file).unwrap().read_to_string(&mut buf).unwrap();
    /// assert_eq!(&buf, "[cargo-update]\n\
    ///                   default_features = true\n\
    ///                   features = [\"serde\"]\n");
    /// ```
    pub fn write(configuration: &BTreeMap<String, PackageConfig>, p: &Path) -> Result<(), i32> {
        File::create(p)
            .map_err(|_| 3)?
            .write_all(&toml::to_vec(configuration).map_err(|_| 2)?)
            .map_err(|_| 3)
    }
}

impl Default for PackageConfig {
    fn default() -> PackageConfig {
        PackageConfig {
            toolchain: None,
            default_features: true,
            features: BTreeSet::new(),
            debug: None,
            install_prereleases: None,
            enforce_lock: None,
            respect_binaries: None,
            target_version: None,
        }
    }
}
