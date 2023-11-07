use std::fmt::{Formatter as FFormatter, Result as FResult, Write as FWrite};
use serde::{Deserializer, Deserialize, Serializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;
use std::default::Default;
use semver::VersionReq;
use std::borrow::Cow;
use std::path::Path;
use serde::de;
use std::fs;
use toml;


/// A single operation to be executed upon configuration of a package.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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
    /// Set environment variable to given value for `cargo install`.
    SetEnvironment(String, String),
    /// Remove environment variable for `cargo install`.
    ClearEnvironment(String),
    /// Remove configuration for an environment variable.
    InheritEnvironment(String),
    /// Reset configuration to default values.
    ResetConfig,
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
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Environment variables to alter for cargo. `None` to remove.
    pub environment: Option<BTreeMap<String, EnvironmentOverride>>,
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
    /// # use cargo_update::ops::{EnvironmentOverride, ConfigOperation, PackageConfig};
    /// # use std::collections::BTreeSet;
    /// # use std::collections::BTreeMap;
    /// # use semver::VersionReq;
    /// # use std::str::FromStr;
    /// assert_eq!(PackageConfig::from(&[ConfigOperation::SetToolchain("nightly".to_string()),
    ///                                  ConfigOperation::DefaultFeatures(false),
    ///                                  ConfigOperation::AddFeature("rustc-serialize".to_string()),
    ///                                  ConfigOperation::SetDebugMode(true),
    ///                                  ConfigOperation::SetInstallPrereleases(false),
    ///                                  ConfigOperation::SetEnforceLock(true),
    ///                                  ConfigOperation::SetRespectBinaries(true),
    ///                                  ConfigOperation::SetTargetVersion(VersionReq::from_str(">=0.1").unwrap()),
    ///                                  ConfigOperation::SetEnvironment("RUSTC_WRAPPER".to_string(), "sccache".to_string()),
    ///                                  ConfigOperation::ClearEnvironment("CC".to_string())]),
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
    ///                environment: Some({
    ///                    let mut vars = BTreeMap::new();
    ///                    vars.insert("RUSTC_WRAPPER".to_string(), EnvironmentOverride(Some("sccache".to_string())));
    ///                    vars.insert("CC".to_string(), EnvironmentOverride(None));
    ///                    vars
    ///                }),
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
    /// let cmd = Command::new("cargo")
    ///               .args(configuration.get(&name).unwrap().cargo_args(&["racer"]).iter().map(AsRef::as_ref))
    ///               .arg(&name)
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
                res.push(if x.ends_with(".exe") {
                        &x[..x.len() - 4]
                    } else {
                        x
                    }
                    .to_string()
                    .into());
            }
        }
        if let Some(true) = self.debug {
            res.push("--debug".into());
        }
        res
    }

    /// Apply transformations from `self.environment` to `cmd`.
    pub fn environmentalise<'c>(&self, cmd: &'c mut Command) -> &'c mut Command {
        if let Some(env) = self.environment.as_ref() {
            for (var, val) in env {
                dbg!((var, val));
                match val {
                    EnvironmentOverride(Some(val)) => cmd.env(var, val),
                    EnvironmentOverride(None) => cmd.env_remove(var),
                };
            }
        }
        cmd
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
    ///     environment: None,
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
    ///                environment: None,
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
            ConfigOperation::SetEnvironment(ref var, ref val) => {
                self.environment.get_or_insert(Default::default()).insert(var.clone(), EnvironmentOverride(Some(val.clone())));
            }
            ConfigOperation::ClearEnvironment(ref var) => {
                self.environment.get_or_insert(Default::default()).insert(var.clone(), EnvironmentOverride(None));
            }
            ConfigOperation::InheritEnvironment(ref var) => {
                self.environment.get_or_insert(Default::default()).remove(var);
            }
            ConfigOperation::ResetConfig => *self = Default::default(),
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
    /// # use std::fs::{self, create_dir_all};
    /// # use std::env::temp_dir;
    /// # use std::io::Write;
    /// # let td = temp_dir().join("cargo_update-doctest").join("PackageConfig-read-0");
    /// # create_dir_all(&td).unwrap();
    /// # let config_file = td.join(".install_config.toml");
    /// fs::write(&config_file, &b"\
    ///    [cargo-update]\n\
    ///    default_features = true\n\
    ///    features = [\"serde\"]\n"[..]).unwrap();
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
    ///         environment: None,
    ///     });
    ///     pkgs
    /// }));
    /// ```
    pub fn read(p: &Path) -> Result<BTreeMap<String, PackageConfig>, (String, i32)> {
        if p.exists() {
            toml::from_str(&fs::read_to_string(p).map_err(|e| (e.to_string(), 1))?).map_err(|e| (e.to_string(), 2))
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
    /// # use std::fs::{self, create_dir_all};
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
    ///         environment: None,
    ///     });
    ///     pkgs
    /// }, &config_file).unwrap();
    ///
    /// assert_eq!(&fs::read_to_string(&config_file).unwrap(),
    ///            "[cargo-update]\n\
    ///             default_features = true\n\
    ///             features = [\"serde\"]\n");
    /// ```
    pub fn write(configuration: &BTreeMap<String, PackageConfig>, p: &Path) -> Result<(), (String, i32)> {
        fs::write(p, &toml::to_vec(configuration).map_err(|e| (e.to_string(), 2))?).map_err(|e| (e.to_string(), 3))
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
            environment: None,
        }
    }
}


/// Wrapper that serialises `None` as a boolean.
///
/// serde's default `BTreeMap<String, Option<String>>` implementation simply loses `None` values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct EnvironmentOverride(pub Option<String>);

impl<'de> Deserialize<'de> for EnvironmentOverride {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(EnvironmentOverrideVisitor)
    }
}

impl Serialize for EnvironmentOverride {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Some(data) => serializer.serialize_str(&data),
            None => serializer.serialize_bool(false),
        }
    }
}

struct EnvironmentOverrideVisitor;

impl<'de> de::Visitor<'de> for EnvironmentOverrideVisitor {
    type Value = EnvironmentOverride;

    fn expecting(&self, formatter: &mut FFormatter) -> FResult {
        write!(formatter, "A string or boolean")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self::Value, E> {
        Ok(EnvironmentOverride(None))
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<Self::Value, E> {
        Ok(EnvironmentOverride(Some(s.to_string())))
    }
}
