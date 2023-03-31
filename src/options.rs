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
use std::ffi::{OsString, OsStr};
use array_tool::vec::Uniq;
use std::fmt::Arguments;
use std::process::exit;
use std::path::PathBuf;
use std::str::FromStr;
use dirs::home_dir;
use std::{env, fs};


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// (Additional) packages to update. Default: `[]`
    pub to_update: Vec<(String, Option<Semver>, String)>,
    /// Whether to update all packages. Default: `false`
    pub all: bool,
    /// Whether to update packages or just list them. Default: `true`
    pub update: bool,
    /// Whether to allow for just installing packages. Default: `false`
    pub install: bool,
    /// Update all packages. Default: `false`
    pub force: bool,
    /// Downdate packages to match newest unyanked registry version.
    pub downdate: bool,
    /// Update git packages too (it's expensive). Default: `false`
    pub update_git: bool,
    /// Don't output messages and pass --quiet to `cargo` subprocesses. Default: `false`
    pub quiet: bool,
    /// Update all packages. Default: empty
    pub filter: Vec<PackageFilterElement>,
    /// The `cargo` home directory. Default: `"$CARGO_INSTALL_ROOT"`, then `"$CARGO_HOME"`, then `"$HOME/.cargo"`
    pub cargo_dir: PathBuf,
    /// The temporary directory to clone git repositories to. Default: `"$TEMP/cargo-update"`
    pub temp_dir: PathBuf,
    /// Arbitrary arguments to forward to `cargo install`, acquired from `$CARGO_INSTALL_OPTS`. Default: `[]`
    pub cargo_install_args: Vec<OsString>,
    /// The cargo to run for installations. Default: `None` (use "cargo")
    pub install_cargo: Option<OsString>,
}

/// Representation of the config application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ConfigOptions {
    /// The `cargo` home directory. Default: `"$CARGO_INSTALL_ROOT"`, then `"$CARGO_HOME"`, then `"$HOME/.cargo"`
    pub cargo_dir: PathBuf,
    /// Crate to modify config for
    pub package: String,
    /// What to do to the config, or display with empty
    pub ops: Vec<ConfigOperation>,
}


impl Options {
    /// Parse `env`-wide command-line arguments into an `Options` instance
    pub fn parse() -> Options {
        let matches = App::new("cargo")
            .bin_name("cargo")
            .version(crate_version!())
            .settings(&[AppSettings::ColoredHelp, AppSettings::ArgRequiredElseHelp, AppSettings::GlobalVersion, AppSettings::SubcommandRequired])
            .subcommand(SubCommand::with_name("install-update")
                .version(crate_version!())
                .author(crate_authors!("\n"))
                .about("A cargo subcommand for checking and applying updates to installed executables")
                .args(&[Arg::from_usage("-c --cargo-dir=[CARGO_DIR] 'The cargo home directory. Default: $CARGO_HOME or $HOME/.cargo'")
                            .visible_alias("root")
                            .allow_invalid_utf8(true)
                            .validator(|s| existing_dir_validator("Cargo", &s)),
                        Arg::from_usage("-t --temp-dir=[TEMP_DIR] 'The temporary directory. Default: $TEMP/cargo-update'")
                            .validator(|s| existing_dir_validator("Temporary", &s)),
                        Arg::from_usage("-a --all 'Update all packages'"),
                        Arg::from_usage("-l --list 'Don't update packages, only list and check if they need an update (all packages by default)'"),
                        Arg::from_usage("-f --force 'Update all packages regardless if they need updating'"),
                        Arg::from_usage("-d --downdate 'Downdate packages to match latest unyanked registry version'"),
                        Arg::from_usage("-i --allow-no-update 'Allow for fresh-installing packages'"),
                        Arg::from_usage("-g --git 'Also update git packages'"),
                        Arg::from_usage("-q --quiet 'No output printed to stdout'"),
                        Arg::from_usage("-s --filter=[PACKAGE_FILTER]... 'Specify a filter a package must match to be considered'")
                            .number_of_values(1)
                            .validator(|s| PackageFilterElement::parse(&s).map(|_| ())),
                        Arg::from_usage("-r --install-cargo=[EXECUTABLE] 'Specify an alternative cargo to run for installations'")
                            .allow_invalid_utf8(true),
                        Arg::with_name("cargo_install_opts")
                            .long("__cargo_install_opts")
                            .env("CARGO_INSTALL_OPTS")
                            .allow_invalid_utf8(true)
                            .empty_values(false)
                            .multiple(true)
                            .value_delimiter(' ')
                            .hidden(true),
                        Arg::from_usage("[PACKAGE]... 'Packages to update'")
                            .empty_values(false)
                            .min_values(1)
                            .validator(|s| package_parse(s).map(|_| ()))]))
            .get_matches();
        let matches = matches.subcommand_matches("install-update").unwrap();

        let all = matches.is_present("all");
        let update = !matches.is_present("list");
        Options {
            to_update: match (all || !update, matches.values_of("PACKAGE")) {
                (_, Some(pkgs)) => {
                    let packages: Vec<_> = pkgs.map(package_parse).map(Result::unwrap).collect();
                    packages.unique_via(|l, r| l.0 == r.0)
                }
                (true, None) => vec![],
                (false, None) => clerror(format_args!("Need at least one PACKAGE without --all")),
            },
            all: all,
            update: update,
            install: matches.is_present("allow-no-update"),
            force: matches.is_present("force"),
            downdate: matches.is_present("downdate"),
            update_git: matches.is_present("git"),
            quiet: matches.is_present("quiet"),
            filter: matches.values_of("filter").map(|pfs| pfs.flat_map(PackageFilterElement::parse).collect()).unwrap_or_else(|| vec![]),
            cargo_dir: cargo_dir(matches.value_of_os("cargo-dir")),
            temp_dir: {
                if let Some(tmpdir) = matches.value_of("temp-dir") {
                    fs::canonicalize(tmpdir).unwrap().join("cargo-update")
                } else {
                    env::temp_dir().join("cargo-update")
                }
            },
            cargo_install_args: matches.values_of_os("cargo_install_opts").into_iter().flat_map(|cio| cio.map(OsStr::to_os_string)).collect(),
            install_cargo: matches.value_of_os("install-cargo").map(OsStr::to_os_string),
        }
    }
}

impl ConfigOptions {
    /// Parse `env`-wide command-line arguments into a `ConfigOptions` instance
    pub fn parse() -> ConfigOptions {
        let matches = App::new("cargo")
            .bin_name("cargo")
            .version(crate_version!())
            .settings(&[AppSettings::ColoredHelp, AppSettings::ArgRequiredElseHelp, AppSettings::GlobalVersion, AppSettings::SubcommandRequired])
            .subcommand(SubCommand::with_name("install-update-config")
                .version(crate_version!())
                .author(crate_authors!("\n"))
                .about("A cargo subcommand for checking and applying updates to installed executables -- configuration")
                .args(&[Arg::from_usage("-c --cargo-dir=[CARGO_DIR] 'The cargo home directory. Default: $CARGO_HOME or $HOME/.cargo'")
                            .validator(|s| existing_dir_validator("Cargo", &s)),
                        Arg::from_usage("-t --toolchain=[TOOLCHAIN] 'Toolchain to use or empty for default'"),
                        Arg::from_usage("-f --feature=[FEATURE]... 'Feature to enable'").number_of_values(1),
                        Arg::from_usage("-n --no-feature=[DISABLED_FEATURE]... 'Feature to disable'").number_of_values(1),
                        Arg::from_usage("-d --default-features=[DEFAULT_FEATURES] 'Whether to allow default features'")
                            .possible_values(&["1", "yes", "true", "0", "no", "false"])
                            .hide_possible_values(true),
                        Arg::from_usage("--debug 'Compile the package in debug mode'").conflicts_with("release"),
                        Arg::from_usage("--release 'Compile the package in release mode'").conflicts_with("debug"),
                        Arg::from_usage("--install-prereleases 'Install prerelease versions'").conflicts_with("no-install-prereleases"),
                        Arg::from_usage("--no-install-prereleases 'Filter out prerelease versions'").conflicts_with("install-prereleases"),
                        Arg::from_usage("--enforce-lock 'Require Cargo.lock to be up to date'").conflicts_with("no-enforce-lock"),
                        Arg::from_usage("--no-enforce-lock 'Don't enforce Cargo.lock'").conflicts_with("enforce-lock"),
                        Arg::from_usage("--respect-binaries 'Only install already installed binaries'").conflicts_with("no-respect-binaries"),
                        Arg::from_usage("--no-respect-binaries 'Install all binaries'").conflicts_with("respect-binaries"),
                        Arg::from_usage("-v --version=[VERSION_REQ] 'Require a cargo-compatible version range'")
                            .validator(|s| SemverReq::from_str(&s).map(|_| ()).map_err(|e| e.to_string()))
                            .conflicts_with("any-version"),
                        Arg::from_usage("-a --any-version 'Allow any version'").conflicts_with("version"),
                        Arg::from_usage("-r --reset 'Roll back the configuration to the defaults.'"),
                        Arg::from_usage("<PACKAGE> 'Package to configure'").empty_values(false)]))
            .get_matches();
        let matches = matches.subcommand_matches("install-update-config").unwrap();

        ConfigOptions {
            cargo_dir: cargo_dir(matches.value_of_os("cargo-dir")),
            package: matches.value_of("PACKAGE").unwrap().to_string(),
            ops: matches.value_of("toolchain")
                .map(|t| if t.is_empty() {
                    ConfigOperation::RemoveToolchain
                } else {
                    ConfigOperation::SetToolchain(t.to_string())
                })
                .into_iter()
                .chain(matches.values_of("feature").into_iter().flatten().map(str::to_string).map(ConfigOperation::AddFeature))
                .chain(matches.values_of("no-feature").into_iter().flatten().map(str::to_string).map(ConfigOperation::RemoveFeature))
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
                .chain(match (matches.is_present("enforce-lock"), matches.is_present("no-enforce-lock")) {
                    (true, _) => Some(ConfigOperation::SetEnforceLock(true)),
                    (_, true) => Some(ConfigOperation::SetEnforceLock(false)),
                    _ => None,
                })
                .chain(match (matches.is_present("respect-binaries"), matches.is_present("no-respect-binaries")) {
                    (true, _) => Some(ConfigOperation::SetRespectBinaries(true)),
                    (_, true) => Some(ConfigOperation::SetRespectBinaries(false)),
                    _ => None,
                })
                .chain(match (matches.is_present("any-version"), matches.value_of("version")) {
                    (true, _) => Some(ConfigOperation::RemoveTargetVersion),
                    (false, Some(vr)) => Some(ConfigOperation::SetTargetVersion(SemverReq::from_str(vr).unwrap())),
                    _ => None,
                })
                .chain(matches.index_of("reset").map(|_| ConfigOperation::ResetConfig))
                .collect(),
        }
    }
}

fn cargo_dir(opt_cargo_dir: Option<&OsStr>) -> PathBuf {
    if let Some(dir) = opt_cargo_dir {
        match fs::canonicalize(dir) {
            Ok(dir) => dir,
            Err(_) => clerror(format_args!("--cargo-dir={:?} doesn't exist", dir)),
        }
    } else {
        match env::var("CARGO_INSTALL_ROOT").map_err(|_| ()).and_then(|ch| fs::canonicalize(ch).map_err(|_| ())) {
            Ok(ch) => ch,
            Err(()) =>
                match env::var("CARGO_HOME").map_err(|_| ()).and_then(|ch| fs::canonicalize(ch).map_err(|_| ())) {
                    Ok(ch) => ch,
                    Err(()) =>
                        match home_dir().and_then(|hd| hd.canonicalize().ok()) {
                            Some(mut hd) => {
                                hd.push(".cargo");
                                fs::create_dir_all(&hd).unwrap();
                                hd
                            }
                            None => clerror(format_args!("$CARGO_INSTALL_ROOT, $CARGO_HOME, and home directory invalid, \
                                                          please specify the cargo home directory with the -c option")),
                        },
                },
        }
    }
}

fn existing_dir_validator(label: &str, s: &str) -> Result<(), String> {
    fs::canonicalize(s).map(|_| ()).map_err(|_| format!("{} directory \"{}\" not found", label, s))
}

fn package_parse(s: &str) -> Result<(String, Option<Semver>, String), String> {
    let mut registry_url = None;
    let mut s = &s[..];
    if s.starts_with('(') {
        if let Some(idx) = s.find("):") {
            registry_url = Some(s[1..idx].to_string());
            s = &s[idx + 2..];
        }
    }

    let registry_url = registry_url.unwrap_or_else(|| "https://github.com/rust-lang/crates.io-index".to_string());

    if let Some(idx) = s.find(':') {
        Ok((s[0..idx].to_string(),
            Some(Semver::parse(&s[idx + 1..]).map_err(|e| format!("Version {} provided for package {} invalid: {}", &s[idx + 1..], &s[0..idx], e))?),
            registry_url))
    } else {
        Ok((s.to_string(), None, registry_url))
    }
}


fn clerror(f: Arguments) -> ! {
    eprintln!("{}", f);
    exit(1)
}
