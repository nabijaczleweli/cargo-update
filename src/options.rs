//! Option parsing and management.
//!
//! Use the `Options::parse()` function to get the program's configuration,
//! as parsed from the commandline.
//!
//! # Examples
//!
//! ```no_run
//! # use cargo_update::Options;
//! let opts = Options::parse();
//! println!("{:#?}", opts);
//! ```


use self::super::ops::{PackageFilterElement, ConfigOperation};
use semver::{VersionReq as SemverReq, Version as Semver};
use clap::{self, AppSettings, SubCommand, App, Arg};
use array_tool::vec::Uniq;
use std::path::PathBuf;
use std::str::FromStr;
use dirs::home_dir;
use std::env;
use std::fs;


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// (Additional) packages to update. Default: `[]`
    pub to_update: Vec<(String, Option<Semver>)>,
    /// Whether to update all packages. Default: `false`
    pub all: bool,
    /// Whether to update packages or just list them. Default: `true`
    pub update: bool,
    /// Whether to allow for just installing packages. Default: `false`
    pub install: bool,
    /// Update all packages. Default: `false`
    pub force: bool,
    /// Update git packages too (it's expensive). Default: `false`
    pub update_git: bool,
    /// Update all packages. Default: empty
    pub filter: Vec<PackageFilterElement>,
    /// The `.crates.toml` file in the `cargo` home directory.
    /// Default: in `"$CARGO_INSTALL_ROOT"`, then `"$CARGO_HOME"`, then `"$HOME/.cargo"`
    pub crates_file: (String, PathBuf),
    /// The `cargo` home directory. Default: `"$CARGO_HOME"`, then `"$HOME/.cargo"`
    pub cargo_dir: (String, PathBuf),
    /// The temporary directory to clone git repositories to. Default: `"$TEMP/cargo-update"`
    pub temp_dir: (String, PathBuf),
}

/// Representation of the config application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ConfigOptions {
    /// The `config` file in the `cargo` home directory.
    /// Default: in `"$CARGO_INSTALL_ROOT"`, then `"$CARGO_HOME"`, then `"$HOME/.cargo"`
    pub crates_file: (String, PathBuf),
    /// Crate to modify config for
    pub package: String,
    /// What to do to the config, or display with empty
    pub ops: Vec<ConfigOperation>,
}


impl Options {
    /// Parse `env`-wide command-line arguments into an `Options` instance
    pub fn parse() -> Options {
        let matches = App::new("cargo")
            .settings(&[AppSettings::ColoredHelp, AppSettings::ArgRequiredElseHelp, AppSettings::GlobalVersion, AppSettings::SubcommandRequired])
            .subcommand(SubCommand::with_name("install-update")
                .version(crate_version!())
                .author(crate_authors!("\n"))
                .about("A cargo subcommand for checking and applying updates to installed executables")
                .args(&[Arg::from_usage("-c --cargo-dir=[CARGO_DIR] 'The cargo home directory. Default: $CARGO_HOME or $HOME/.cargo'")
                            .visible_alias("root")
                            .validator(|s| existing_dir_validator("Cargo", &s)),
                        Arg::from_usage("-t --temp-dir=[TEMP_DIR] 'The temporary directory. Default: $TEMP/cargo-update'")
                            .validator(|s| existing_dir_validator("Temporary", &s)),
                        Arg::from_usage("-a --all 'Update all packages'"),
                        Arg::from_usage("-l --list 'Don't update packages, only list and check if they need an update (all packages by default)'"),
                        Arg::from_usage("-f --force 'Update all packages regardless if they need updating'"),
                        Arg::from_usage("-i --allow-no-update 'Allow for fresh-installing packages'"),
                        Arg::from_usage("-g --git 'Also update git packages'"),
                        Arg::from_usage("-s --filter=[PACKAGE_FILTER]... 'Specify a filter a package must match to be considered'")
                            .validator(|s| PackageFilterElement::parse(&s).map(|_| ())),
                        Arg::from_usage("[PACKAGE]... 'Packages to update'")
                            .empty_values(false)
                            .min_values(1)
                            .validator(|s| package_parse(s).map(|_| ()))]))
            .get_matches();
        let matches = matches.subcommand_matches("install-update").unwrap();

        let all = matches.is_present("all");
        let update = !matches.is_present("list");
        let cdir = cargo_dir();
        Options {
            to_update: match (all || !update, matches.values_of("PACKAGE")) {
                (_, Some(pkgs)) => {
                    let packages: Vec<_> = pkgs.map(String::from).map(package_parse).map(Result::unwrap).collect();
                    packages.unique_via(|l, r| l.0 == r.0)
                }
                (true, None) => vec![],
                (false, None) => {
                    clap::Error {
                            message: format!("Need at least one PACKAGE without --all"),
                            kind: clap::ErrorKind::MissingRequiredArgument,
                            info: None,
                        }
                        .exit()
                }
            },
            all: all,
            update: update,
            install: matches.is_present("allow-no-update"),
            force: matches.is_present("force"),
            update_git: matches.is_present("git"),
            filter: matches.values_of("filter").map(|pfs| pfs.flat_map(PackageFilterElement::parse).collect()).unwrap_or_else(|| vec![]),
            crates_file: match matches.value_of("cargo-dir") {
                Some(dir) => (format!("{}/.crates.toml", dir), fs::canonicalize(dir).unwrap().join(".crates.toml")),
                None => {
                    match env::var("CARGO_INSTALL_ROOT").map_err(|_| ()).and_then(|ch| fs::canonicalize(ch).map_err(|_| ())) {
                        Ok(ch) => ("$CARGO_INSTALL_ROOT/.crates.toml".to_string(), ch.join(".crates.toml")),
                        Err(()) => (format!("{}/.crates.toml", cdir.0), cdir.1.join(".crates.toml")),
                    }
                }
            },
            cargo_dir: cdir,
            temp_dir: {
                let (temp_s, temp_pb) = if let Some(tmpdir) = matches.value_of("temp-dir") {
                    (tmpdir.to_string(), fs::canonicalize(tmpdir).unwrap())
                } else {
                    ("$TEMP".to_string(), env::temp_dir())
                };

                (format!("{}{}cargo-update",
                         temp_s,
                         if temp_s.ends_with('/') || temp_s.ends_with('\\') {
                             ""
                         } else {
                             "/"
                         }),
                 temp_pb.join("cargo-update"))
            },
        }
    }
}

impl ConfigOptions {
    /// Parse `env`-wide command-line arguments into a `ConfigOptions` instance
    pub fn parse() -> ConfigOptions {
        let matches = App::new("cargo")
            .settings(&[AppSettings::ColoredHelp, AppSettings::ArgRequiredElseHelp, AppSettings::GlobalVersion, AppSettings::SubcommandRequired])
            .subcommand(SubCommand::with_name("install-update-config")
                .version(crate_version!())
                .author(crate_authors!("\n"))
                .about("A cargo subcommand for checking and applying updates to installed executables -- configuration")
                .args(&[Arg::from_usage("-c --cargo-dir=[CARGO_DIR] 'The cargo home directory. Default: $CARGO_HOME or $HOME/.cargo'")
                            .validator(|s| existing_dir_validator("Cargo", &s)),
                        Arg::from_usage("-t --toolchain=[TOOLCHAIN] 'Toolchain to use or empty for default'"),
                        Arg::from_usage("-f --feature=[FEATURE]... 'Feature to enable'"),
                        Arg::from_usage("-n --no-feature=[DISABLED_FEATURE]... 'Feature to disable'"),
                        Arg::from_usage("-d --default-features=[DEFAULT_FEATURES] 'Whether to allow default features'")
                            .possible_values(&["1", "yes", "true", "0", "no", "false"])
                            .hide_possible_values(true),
                        Arg::from_usage("--debug 'Compile the package in debug mode'").conflicts_with("release"),
                        Arg::from_usage("--release 'Compile the package in release mode'").conflicts_with("debug"),
                        Arg::from_usage("--install-prereleases 'Install prerelease versions'").conflicts_with("no-install-prereleases"),
                        Arg::from_usage("--no-install-prereleases 'Filter out prerelease versions'").conflicts_with("install-prereleases"),
                        Arg::from_usage("-v --version=[VERSION_REQ] 'Require a cargo-compatible version range'")
                            .validator(|s| SemverReq::from_str(&s).map(|_| ()).map_err(|e| e.to_string()))
                            .conflicts_with("any-version"),
                        Arg::from_usage("-a --any-version 'Allow any version'").conflicts_with("version"),
                        Arg::from_usage("<PACKAGE> 'Package to configure'").empty_values(false)]))
            .get_matches();
        let matches = matches.subcommand_matches("install-update-config").unwrap();

        let cdir = cargo_dir();
        ConfigOptions {
            crates_file: match matches.value_of("cargo-dir") {
                Some(dir) => (format!("{}/.crates.toml", dir), fs::canonicalize(dir).unwrap().join(".crates.toml")),
                None => {
                    match env::var("CARGO_INSTALL_ROOT").map_err(|_| ()).and_then(|ch| fs::canonicalize(ch).map_err(|_| ())) {
                        Ok(ch) => ("$CARGO_INSTALL_ROOT/.crates.toml".to_string(), ch.join(".crates.toml")),
                        Err(()) => (format!("{}/.crates.toml", cdir.0), cdir.1.join(".crates.toml")),
                    }
                }
            },
            package: matches.value_of("PACKAGE").unwrap().to_string(),
            ops: matches.value_of("toolchain")
                .map(|t| if t.is_empty() {
                    ConfigOperation::RemoveToolchain
                } else {
                    ConfigOperation::SetToolchain(t.to_string())
                })
                .into_iter()
                .chain(matches.values_of("feature").into_iter().flat_map(|f| f).map(str::to_string).map(ConfigOperation::AddFeature))
                .chain(matches.values_of("no-feature").into_iter().flat_map(|f| f).map(str::to_string).map(ConfigOperation::RemoveFeature))
                .chain(matches.value_of("default-features").map(|d| ["1", "yes", "true"].contains(&d)).map(ConfigOperation::DefaultFeatures).into_iter())
                .chain(match (matches.is_present("debug"), matches.is_present("release")) {
                    (true, _) => Some(ConfigOperation::SetDebugMode(true)),
                    (_, true) => Some(ConfigOperation::SetDebugMode(false)),
                    _ => None,
                })
                .chain(match (matches.is_present("install-prereleases"), matches.is_present("no-install-prereleases")) {
                    (true, _) => Some(ConfigOperation::SetInstallPrereleases(true)),
                    (_, true) => Some(ConfigOperation::SetInstallPrereleases(false)),
                    _ => None,
                })
                .chain(match (matches.is_present("any-version"), matches.value_of("version")) {
                    (true, _) => Some(ConfigOperation::RemoveTargetVersion),
                    (false, Some(vr)) => Some(ConfigOperation::SetTargetVersion(SemverReq::from_str(vr).unwrap())),
                    _ => None,
                })
                .collect(),
        }
    }
}

fn cargo_dir() -> (String, PathBuf) {
    match env::var("CARGO_HOME").map_err(|_| ()).and_then(|ch| fs::canonicalize(ch).map_err(|_| ())) {
        Ok(ch) => ("$CARGO_HOME".to_string(), ch),
        Err(()) =>
                match home_dir().and_then(|hd| hd.canonicalize().ok()) {
                    Some(mut hd) => {
                        hd.push(".cargo");

                        fs::create_dir_all(&hd).unwrap();
                        ("$HOME/.cargo".to_string(), hd)
                    }
                    None => {
                        clap::Error {
                                message: "$CARGO_HOME and home directory invalid, please specify the cargo home directory with the -c option".to_string(),
                                kind: clap::ErrorKind::MissingRequiredArgument,
                                info: None,
                            }
                            .exit()
                    }
                },
    }
}

fn existing_dir_validator(label: &str, s: &str) -> Result<(), String> {
    fs::canonicalize(s).map(|_| ()).map_err(|_| format!("{} directory \"{}\" not found", label, s))
}

fn package_parse(s: String) -> Result<(String, Option<Semver>), String> {
    if let Some(idx) = s.find(':') {
        Ok((s[0..idx].to_string(),
            Some(Semver::parse(&s[idx + 1..]).map_err(|e| format!("Version {} provided for package {} invalid: {}", &s[idx + 1..], &s[0..idx], e))?)))
    } else {
        Ok((s, None))
    }
}
