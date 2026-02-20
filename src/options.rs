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
use clap::{AppSettings, SubCommand, App, Arg};
use std::num::{ParseIntError, NonZero};
use std::ffi::{OsString, OsStr};
use std::path::{PathBuf, Path};
use std::fmt::Arguments;
use whoami::username_os;
use std::process::exit;
use std::str::FromStr;
use std::borrow::Cow;
use std::{env, fs};
use home;


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// (Additional) packages to update. Default: `[]`
    pub to_update: Vec<(String, Option<Semver>, Cow<'static, str>)>,
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
    /// Enforce packages' embedded `Cargo.lock`. Exactly like `CARGO_INSTALL_OPTS=--locked` (or `--enforce-lock` per package)
    /// except doesn't disable cargo-binstall. Default: `false`
    pub locked: bool,
    /// Update all packages. Default: empty
    pub filter: Vec<PackageFilterElement>,
    /// The `cargo` home directory; (original, canonicalised). Default: `"$CARGO_INSTALL_ROOT"`, then `"$CARGO_HOME"`,
    /// then `"$HOME/.cargo"`
    pub cargo_dir: (PathBuf, PathBuf),
    /// The temporary directory to clone git repositories to. Default: `"$TEMP/cargo-update"`
    pub temp_dir: PathBuf,
    /// Arbitrary arguments to forward to `cargo install`, acquired from `$CARGO_INSTALL_OPTS`. Default: `[]`
    pub cargo_install_args: Vec<OsString>,
    /// The cargo to run for installations. Default: `None` (use "cargo")
    pub install_cargo: Option<OsString>,
    /// `cargo install -j` argument. Default: `None`
    pub jobs: Option<NonZero<usize>>,
    /// Start jobserver to fill this many CPUs. Default: `None`
    pub recursive_jobs: Option<NonZero<usize>>,
    /// Additional limit of concurrent `cargo install`s. Default: `None`
    pub concurrent_cargos: Option<NonZero<usize>>,
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
        let nproc = std::thread::available_parallelism().unwrap_or(NonZero::new(1).unwrap());
        let matches = App::new("cargo")
            .bin_name("cargo")
            .version(crate_version!())
            .settings(&[AppSettings::ColoredHelp, AppSettings::ArgRequiredElseHelp, AppSettings::GlobalVersion, AppSettings::SubcommandRequired])
            .subcommand(SubCommand::with_name("install-update")
                .version(crate_version!())
                .author("https://github.com/nabijaczleweli/cargo-update")
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
                        Arg::from_usage("--locked 'Enforce packages' embedded Cargo.lock'"),
                        Arg::from_usage("-s --filter=[PACKAGE_FILTER]... 'Specify a filter a package must match to be considered'")
                            .number_of_values(1)
                            .validator(|s| PackageFilterElement::parse(&s).map(|_| ())),
                        Arg::from_usage("-r --install-cargo=[EXECUTABLE] 'Specify an alternative cargo to run for installations'")
                            .number_of_values(1)
                            .allow_invalid_utf8(true),
                        Arg::from_usage(&format!("-j --jobs=[JOBS] 'Limit number of parallel jobs or \"default\" for {}'", nproc))
                            .number_of_values(1)
                            .validator(|s| jobs_parse(s, "default", nproc)),
                        Arg::from_usage(&format!("-J --recursive-jobs=[JOBS] 'Build up to JOBS crates at once on up to JOBS CPUs. {} if empty.'",
                                                 nproc))
                            .number_of_values(1)
                            .forbid_empty_values(false)
                            .default_missing_value("")
                            .validator(|s| jobs_parse(s, "", nproc)),
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
        let jobs_arg = matches.value_of("jobs").map(|j| jobs_parse(j, "default", nproc).unwrap());
        let recursive_jobs = matches.value_of("recursive-jobs").map(|rj| jobs_parse(rj, "", nproc).unwrap());
        Options {
            to_update: match (all || !update, matches.values_of("PACKAGE")) {
                (_, Some(pkgs)) => {
                    let mut packages: Vec<_> = pkgs.map(package_parse)
                        .map(Result::unwrap)
                        .map(|(package, version, registry)| {
                            (package.to_string(),
                             version,
                             registry.map(str::to_string).map(Cow::from).unwrap_or("https://github.com/rust-lang/crates.io-index".into()))
                        })
                        .collect();
                    packages.sort_by(|l, r| l.0.cmp(&r.0));
                    packages.dedup_by(|l, r| l.0 == r.0);
                    packages
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
            locked: matches.is_present("locked"),
            filter: matches.values_of("filter").map(|pfs| pfs.flat_map(PackageFilterElement::parse).collect()).unwrap_or_default(),
            cargo_dir: cargo_dir(matches.value_of_os("cargo-dir")),
            temp_dir: {
                if let Some(tmpdir) = matches.value_of("temp-dir") {
                        fs::canonicalize(tmpdir).unwrap()
                    } else {
                        env::temp_dir()
                    }
                    .join(Path::new("cargo-update").with_extension(username_os()))
            },
            cargo_install_args: matches.values_of_os("cargo_install_opts").into_iter().flat_map(|cio| cio.map(OsStr::to_os_string)).collect(),
            install_cargo: matches.value_of_os("install-cargo").map(OsStr::to_os_string),
            jobs: if recursive_jobs.is_some() {
                None
            } else {
                jobs_arg
            },
            recursive_jobs: recursive_jobs,
            concurrent_cargos: match (jobs_arg, recursive_jobs) {
                (Some(j), Some(rj)) => Some(NonZero::new((rj.get() + (j.get() - 1)) / j).unwrap_or(NonZero::new(1).unwrap())),
                _ => None,
            },
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
                .author("https://github.com/nabijaczleweli/cargo-update")
                .about("A cargo subcommand for checking and applying updates to installed executables -- configuration")
                .args(&[Arg::from_usage("-c --cargo-dir=[CARGO_DIR] 'The cargo home directory. Default: $CARGO_HOME or $HOME/.cargo'")
                            .validator(|s| existing_dir_validator("Cargo", &s)),
                        Arg::from_usage("-t --toolchain=[TOOLCHAIN] 'Toolchain to use or empty for default'"),
                        Arg::from_usage("-f --feature=[FEATURE]... 'Feature to enable'").number_of_values(1),
                        Arg::from_usage("-n --no-feature=[DISABLED_FEATURE]... 'Feature to disable'").number_of_values(1),
                        Arg::from_usage("-d --default-features=[DEFAULT_FEATURES] 'Whether to allow default features'")
                            .possible_values(&["1", "yes", "true", "0", "no", "false"])
                            .hide_possible_values(true),
                        Arg::from_usage("--debug 'Compile the package in debug (\"dev\") mode'").conflicts_with("release").conflicts_with("build-profile"),
                        Arg::from_usage("--release 'Compile the package in release mode'").conflicts_with("debug").conflicts_with("build-profile"),
                        Arg::from_usage("--build-profile=[PROFILE] 'Compile the package in the given profile'")
                            .conflicts_with("debug")
                            .conflicts_with("release"),
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
                        Arg::from_usage("-e --environment=[VARIABLE=VALUE]... 'Environment variable to set'")
                            .number_of_values(1)
                            .validator(|s| if s.contains('=') {
                                Ok(())
                            } else {
                                Err("Missing VALUE")
                            }),
                        Arg::from_usage("-E --clear-environment=[VARIABLE]... 'Environment variable to clear'")
                            .number_of_values(1)
                            .validator(|s| if s.contains('=') {
                                Err("VARIABLE can't contain a =")
                            } else {
                                Ok(())
                            }),
                        Arg::from_usage("--inherit-environment=[VARIABLE]... 'Environment variable to use from the environment'")
                            .number_of_values(1)
                            .validator(|s| if s.contains('=') {
                                Err("VARIABLE can't contain a =")
                            } else {
                                Ok(())
                            }),
                        Arg::from_usage("-r --reset 'Roll back the configuration to the defaults.'"),
                        Arg::from_usage("<PACKAGE> 'Package to configure'").empty_values(false)]))
            .get_matches();
        let matches = matches.subcommand_matches("install-update-config").unwrap();

        ConfigOptions {
            cargo_dir: cargo_dir(matches.value_of_os("cargo-dir")).1,
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
                .chain(match (matches.is_present("debug"), matches.is_present("release"), matches.value_of("build-profile")) {
                    (true, _, _) => Some(ConfigOperation::SetBuildProfile("dev".into())),
                    (_, true, _) => Some(ConfigOperation::SetBuildProfile("release".into())),
                    (_, _, Some(prof)) => Some(ConfigOperation::SetBuildProfile(prof.to_string().into())),
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
                .chain(matches.values_of("environment")
                    .into_iter()
                    .flatten()
                    .map(|s| s.split_once('=').unwrap())
                    .map(|(k, v)| ConfigOperation::SetEnvironment(k.to_string(), v.to_string())))
                .chain(matches.values_of("clear-environment").into_iter().flatten().map(str::to_string).map(ConfigOperation::ClearEnvironment))
                .chain(matches.values_of("inherit-environment").into_iter().flatten().map(str::to_string).map(ConfigOperation::InheritEnvironment))
                .chain(matches.index_of("reset").map(|_| ConfigOperation::ResetConfig))
                .collect(),
        }
    }
}

fn cargo_dir(opt_cargo_dir: Option<&OsStr>) -> (PathBuf, PathBuf) {
    if let Some(dir) = opt_cargo_dir {
        match fs::canonicalize(&dir) {
            Ok(cdir) => (dir.into(), cdir),
            Err(_) => clerror(format_args!("--cargo-dir={:?} doesn't exist", dir)),
        }
    } else {
        match env::var_os("CARGO_INSTALL_ROOT")
            .map(PathBuf::from)
            .or_else(|| home::cargo_home().ok())
            .and_then(|ch| fs::canonicalize(&ch).map(|can| (ch, can)).ok()) {
            Some(cd) => cd,
            None => {
                clerror(format_args!("$CARGO_INSTALL_ROOT, $CARGO_HOME, and home directory invalid, please specify the cargo home directory with the -c \
                                      option"))
            }
        }
    }
}

fn existing_dir_validator(label: &str, s: &str) -> Result<(), String> {
    fs::canonicalize(s).map(|_| ()).map_err(|_| format!("{} directory \"{}\" not found", label, s))
}

fn package_parse(mut s: &str) -> Result<(&str, Option<Semver>, Option<&str>), String> {
    let mut registry_url = None;
    if s.starts_with('(') {
        if let Some(idx) = s.find("):") {
            registry_url = Some(&s[1..idx]);
            s = &s[idx + 2..];
        }
    }

    if let Some(idx) = s.find(':') {
        Ok((&s[0..idx],
            Some(Semver::parse(&s[idx + 1..]).map_err(|e| format!("Version {} provided for package {} invalid: {}", &s[idx + 1..], &s[0..idx], e))?),
            registry_url))
    } else {
        Ok((s, None, registry_url))
    }
}

fn jobs_parse(s: &str, special: &str, default: NonZero<usize>) -> Result<NonZero<usize>, ParseIntError> {
    if s != special {
        NonZero::<usize>::from_str(s)
    } else {
        Ok(default)
    }
}


fn clerror(f: Arguments) -> ! {
    eprintln!("{}", f);
    exit(1)
}
