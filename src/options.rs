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


use clap::builder::{ValueParser, TypedValueParser, NonEmptyStringValueParser, PossibleValue};
use clap::error::{Error as ClapError, ErrorKind as ClapErrorKind};
use self::super::ops::{PackageFilterElement, ConfigOperation};
use semver::{VersionReq as SemverReq, Version as Semver};
use chrono::{TimeDelta, DateTime, Utc};
use clap::{Command, Arg, ArgAction};
use std::ffi::{OsString, OsStr};
use std::path::{PathBuf, Path};
use std::fmt::Arguments;
use whoami::username_os;
use std::process::exit;
use std::str::FromStr;
use std::num::NonZero;
use std::borrow::Cow;
use std::{env, fs};
use home;


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// (Additional) packages to update. Default: `[]`
    pub to_update: Vec<(String, Option<Semver>, Cow<'static, str>)>,
    /// Packages to exclude
    pub to_exclude: Vec<String>,
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
    /// Only install versions released after this time. Default: `None`
    pub released_after: Option<DateTime<Utc>>,
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
        let mut matches = Command::new("cargo")
            .bin_name("cargo")
            .version(crate_version!())
            .arg_required_else_help(true)
            .subcommand_required(true)
            .args_override_self(true)
            .subcommand(Command::new("install-update")
                .version(crate_version!())
                .author("https://github.com/nabijaczleweli/cargo-update")
                .about("A cargo subcommand for checking and applying updates to installed executables")
                .args(&[arg!(-c --"cargo-dir" <CARGO_DIR> "The cargo home directory. Default: $CARGO_HOME or $HOME/.cargo")
                            .required(false)
                            .action(ArgAction::Set)
                            .visible_alias("root")
                            .value_parser(ExistingDirParser("Cargo")),
                        arg!(-t --"temp-dir" <TEMP_DIR> "The temporary directory. Default: $TEMP/cargo-update")
                            .required(false)
                            .action(ArgAction::Set)
                            .value_parser(ExistingDirParser("Temporary")),
                        arg!(-a --"all" "Update all packages").action(ArgAction::SetTrue),
                        arg!(-l --"list" "Don't update packages, only list and check if they need an update (all packages by default)")
                            .action(ArgAction::SetTrue),
                        arg!(-f --"force" "Update all packages regardless if they need updating").action(ArgAction::SetTrue),
                        arg!(-d --"downdate" "Downdate packages to match latest unyanked registry version").action(ArgAction::SetTrue),
                        arg!(-i --"allow-no-update" "Allow for fresh-installing packages").action(ArgAction::SetTrue),
                        arg!(-g --"git" "Also update git packages").action(ArgAction::SetTrue),
                        arg!(-q --"quiet" "No output printed to stdout").action(ArgAction::SetTrue),
                        arg!(--"locked" "Enforce packages' embedded Cargo.lock").action(ArgAction::SetTrue),
                        arg!(--"cooldown" <TIME> "Only consider versions released before (now - TIME). Seconds, [smhdwy] suffix.")
                            .required(false)
                            .action(ArgAction::Set)
                            .num_args(1)
                            .value_parser(duration_parse),
                        arg!(-s --"filter" <PACKAGE_FILTER>... "Specify a filter a package must match to be considered")
                            .required(false)
                            .action(ArgAction::Append)
                            .num_args(1)
                            .value_parser(PackageFilterElement::parse),
                        arg!(-x --"exclude" <PACKAGE_NAME>... "Specify package name to exclude")
                            .required(false)
                            .action(ArgAction::Append),
                        arg!(-r --"install-cargo" <EXECUTABLE> "Specify an alternative cargo to run for installations")
                            .required(false)
                            .action(ArgAction::Set)
                            .num_args(1)
                            .value_parser(ValueParser::os_string()),
                        arg!(-j --"jobs" <JOBS>)
                            .help(format!("Limit number of parallel jobs or \"default\" for {}", nproc))
                            .required(false)
                            .num_args(1)
                            .value_parser(JobsParser("default", nproc)),
                        arg!(-J --"recursive-jobs" <JOBS>)
                            .help(format!("Build up to JOBS crates at once on up to JOBS CPUs. {} if empty.", nproc))
                            .required(false)
                            .num_args(1)
                            .default_value("")
                            .value_parser(JobsParser("", nproc)),
                        Arg::new("cargo_install_opts")
                            .long("__cargo_install_opts")
                            .env("CARGO_INSTALL_OPTS")
                            .action(ArgAction::Set)
                            .value_parser(ValueParser::os_string())
                            .value_delimiter(' ')
                            .hide(true),
                        arg!(<PACKAGE>... "Packages to update")
                            .action(ArgAction::Append)
                            .required(false)
                            .value_parser(package_parse)]))
            .get_matches_mut();
        let (_, mut matches) = matches.remove_subcommand().unwrap();

        let all = matches.remove_one("all").unwrap_or(false);
        let update = !matches.remove_one("list").unwrap_or(false);
        let jobs_arg = matches.remove_one("jobs");
        let to_exclude: Vec<String> = matches.get_many::<String>("exclude").unwrap_or_default().cloned().collect();
        let recursive_jobs = matches.remove_one("recursive-jobs");
        Options {
            to_update: match (all || !update, matches.remove_many::<(String, Option<Semver>, Option<String>)>("PACKAGE")) {
                (_, Some(pkgs)) => {
                    let mut packages: Vec<_> = pkgs.map(|(package, version, registry)| {
                            (package, version, registry.map(Cow::from).unwrap_or("https://github.com/rust-lang/crates.io-index".into()))
                        })
                        .collect();
                    packages.sort_by(|l, r| l.0.cmp(&r.0));
                    packages.dedup_by(|l, r| l.0 == r.0);
                    packages
                }
                (true, None) => vec![],
                (false, None) => clerror(format_args!("Need at least one PACKAGE without --all")),
            },
            to_exclude,
            all: all,
            update: update,
            install: matches.remove_one("allow-no-update").unwrap_or(false),
            force: matches.remove_one("force").unwrap_or(false),
            downdate: matches.remove_one("downdate").unwrap_or(false),
            update_git: matches.remove_one("git").unwrap_or(false),
            quiet: matches.remove_one("quiet").unwrap_or(false),
            released_after: matches.get_one::<TimeDelta>("cooldown")
                .map(|&td| {
                    Utc::now().checked_sub_signed(td).unwrap_or_else(|| {
                        let raw = matches.get_raw("cooldown").unwrap_or_default().last().unwrap();
                        clerror(format_args!("--cooldown {}: (now - {}) out of range", Path::new(raw).display(), td)) // TODO: MSRV 1.87 OsStr::display()
                    })
                }),
            locked: matches.remove_one("locked").unwrap_or(false),
            filter: matches.remove_many("filter").into_iter().flatten().collect(),
            cargo_dir: cargo_dir(matches.remove_one("cargo-dir")),
            temp_dir: matches.remove_one("temp-dir").unwrap_or_else(env::temp_dir).join(Path::new("cargo-update").with_extension(username_os())),
            cargo_install_args: matches.remove_many("cargo_install_opts").into_iter().flatten().filter(|a: &OsString| !a.is_empty()).collect(),
            install_cargo: matches.remove_one("install-cargo"),
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
        let mut matches = Command::new("cargo")
            .bin_name("cargo")
            .version(crate_version!())
            // .settings(&[ /* AppSettings::GlobalVersion*/ ])
            .arg_required_else_help(true)
            .subcommand_required(true)
            .subcommand(Command::new("install-update-config")
                .disable_version_flag(true)
                .version(crate_version!())
                .author("https://github.com/nabijaczleweli/cargo-update")
                .about("A cargo subcommand for checking and applying updates to installed executables -- configuration")
                .args(&[arg!(-c --"cargo-dir" <CARGO_DIR> "The cargo home directory. Default: $CARGO_HOME or $HOME/.cargo").required(false)
                            .value_parser(ExistingDirParser("Cargo")),
                        arg!(-t --"toolchain" <TOOLCHAIN> "Toolchain to use or empty for default")
                        .num_args(1).required(false).value_parser(ValueParser::string()),
                        arg!(-f --"feature" <FEATURE>... "Feature to enable").num_args(1).required(false).value_parser(ValueParser::string()),
                        arg!(-n --"no-feature" <DISABLED_FEATURE>... "Feature to disable").num_args(1).required(false).value_parser(ValueParser::string()),
                        arg!(-d --"default-features" <DEFAULT_FEATURES> "Whether to allow default features").num_args(1).required(false)
                            .value_parser(DefaultFeaturesBoolParser)
                            .hide_possible_values(true),
                        arg!(--"debug" "Compile the package in debug (\"dev\") mode")
                        .action(ArgAction::SetTrue).conflicts_with("release").conflicts_with("build-profile"),
                        arg!(--"release" "Compile the package in release mode")
                        .action(ArgAction::SetTrue).conflicts_with("debug").conflicts_with("build-profile"),
                        arg!(--"build-profile" <PROFILE> "Compile the package in the given profile").num_args(1).required(false)
                            .conflicts_with("debug")
                            .conflicts_with("release").value_parser(ValueParser::string()),
                        arg!(--"install-prereleases" "Install prerelease versions").action(ArgAction::SetTrue).conflicts_with("no-install-prereleases"),
                        arg!(--"no-install-prereleases" "Filter out prerelease versions").action(ArgAction::SetTrue).conflicts_with("install-prereleases"),
                        arg!(--"enforce-lock" "Require Cargo.lock to be up to date").action(ArgAction::SetTrue).conflicts_with("no-enforce-lock"),
                        arg!(--"no-enforce-lock" "Don't enforce Cargo.lock").action(ArgAction::SetTrue).conflicts_with("enforce-lock"),
                        arg!(--"respect-binaries" "Only install already installed binaries").action(ArgAction::SetTrue).conflicts_with("no-respect-binaries"),
                        arg!(--"no-respect-binaries" "Install all binaries").action(ArgAction::SetTrue).conflicts_with("respect-binaries"),
                        arg!(-v --"version" <VERSION_REQ> "Require a cargo-compatible version range").num_args(1).required(false)
                            .value_parser(SemverReq::from_str)
                            .conflicts_with("any-version"),
                        arg!(-a --"any-version" "Allow any version").action(ArgAction::SetTrue).conflicts_with("version"),
                        arg!(-e --"environment" <VARIABLE_EQ_TODO_VALUE>... "Environment variable to set").required(false)
                            .num_args(1)
                            .value_parser(|s: &str| if let Some((k,v)) = s.split_once('=') {
                                Ok((k.to_string(), v.to_string()))
                            } else {
                                Err("Missing VALUE")
                            }),
                        arg!(-E --"clear-environment" <VARIABLE>... "Environment variable to clear").required(false)
                            .num_args(1)
                            .value_parser(|s: &str| if s.contains('=') {
                                Err("VARIABLE can't contain a =")
                            } else {
                                Ok(s.to_string())
                            }),
                        arg!(--"inherit-environment" <VARIABLE>... "Environment variable to use from the environment").required(false)
                            .num_args(1)
                            .value_parser(|s: &str| if s.contains('=') {
                                Err("VARIABLE can't contain a =")
                            } else {
                                Ok(s.to_string())
                            }),
                        arg!(-r --"reset" "Roll back the configuration to the defaults.").action(ArgAction::SetTrue),
                        arg!(<PACKAGE> "Package to configure").value_parser(NonEmptyStringValueParser ::new())
                        ]))
            .get_matches_mut();
        let (_, mut matches) = matches.remove_subcommand().unwrap();

        ConfigOptions {
            cargo_dir: cargo_dir(matches.remove_one("cargo-dir")).1,
            package: matches.remove_one("PACKAGE").unwrap(),
            ops: matches.remove_one("toolchain")
                .map(|t: String| if t.is_empty() {
                    ConfigOperation::RemoveToolchain
                } else {
                    ConfigOperation::SetToolchain(t)
                })
                .into_iter()
                .chain(matches.remove_many("feature").into_iter().flatten().map(ConfigOperation::AddFeature))
                .chain(matches.remove_many("no-feature").into_iter().flatten().map(ConfigOperation::RemoveFeature))
                .chain(matches.remove_one("default-features").map(ConfigOperation::DefaultFeatures))
                .chain(match (matches.remove_one("debug").unwrap_or(false),
                              matches.remove_one("release").unwrap_or(false),
                              matches.remove_one::<String>("build-profile")) {
                    (true, _, _) => Some(ConfigOperation::SetBuildProfile("dev".into())),
                    (_, true, _) => Some(ConfigOperation::SetBuildProfile("release".into())),
                    (_, _, Some(prof)) => Some(ConfigOperation::SetBuildProfile(prof.into())),
                    _ => None,
                })
                .chain(match (matches.remove_one("install-prereleases").unwrap_or(false), matches.remove_one("no-install-prereleases").unwrap_or(false)) {
                    (true, _) => Some(ConfigOperation::SetInstallPrereleases(true)),
                    (_, true) => Some(ConfigOperation::SetInstallPrereleases(false)),
                    _ => None,
                })
                .chain(match (matches.remove_one("enforce-lock").unwrap_or(false), matches.remove_one("no-enforce-lock").unwrap_or(false)) {
                    (true, _) => Some(ConfigOperation::SetEnforceLock(true)),
                    (_, true) => Some(ConfigOperation::SetEnforceLock(false)),
                    _ => None,
                })
                .chain(match (matches.remove_one("respect-binaries").unwrap_or(false), matches.remove_one("no-respect-binaries").unwrap_or(false)) {
                    (true, _) => Some(ConfigOperation::SetRespectBinaries(true)),
                    (_, true) => Some(ConfigOperation::SetRespectBinaries(false)),
                    _ => None,
                })
                .chain(match (matches.remove_one("any-version").unwrap_or(false), matches.remove_one("version")) {
                    (true, _) => Some(ConfigOperation::RemoveTargetVersion),
                    (false, Some(vr)) => Some(ConfigOperation::SetTargetVersion(vr)),
                    _ => None,
                })
                .chain(matches.remove_many("environment")
                    .into_iter()
                    .flatten()
                    .map(|(k, v)| ConfigOperation::SetEnvironment(k, v)))
                .chain(matches.remove_many("clear-environment").into_iter().flatten().map(ConfigOperation::ClearEnvironment))
                .chain(matches.remove_many("inherit-environment").into_iter().flatten().map(ConfigOperation::InheritEnvironment))
                .chain(if matches.remove_one("reset").unwrap_or(false) {
                    Some(ConfigOperation::ResetConfig)
                } else {
                    None
                })
                .collect(),
        }
    }
}

fn cargo_dir(opt_cargo_dir: Option<PathBuf>) -> (PathBuf, PathBuf) {
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

#[derive(Copy, Clone)]
struct ExistingDirParser(&'static str);
impl TypedValueParser for ExistingDirParser {
    type Value = PathBuf;

    fn parse_ref(&self, cmd: &Command, arg: Option<&Arg>, value: &OsStr) -> Result<Self::Value, ClapError> {
        fs::canonicalize(value).map_err(|_| {
            ClapError::raw(ClapErrorKind::InvalidValue,
                           format_args!("{}: {} directory \"{}\" not found", arg.unwrap(), self.0, Path::new(value).display())) // TODO: MSRV 1.87 OsStr::display()
                .with_cmd(cmd)
        })
    }
}

fn package_parse(mut s: &str) -> Result<(String, Option<Semver>, Option<String>), String> {
    let mut registry_url = None;
    if s.starts_with('(') {
        if let Some(idx) = s.find("):") {
            registry_url = Some(&s[1..idx]);
            s = &s[idx + 2..];
        }
    }

    if let Some(idx) = s.find(':') {
        Ok((s[0..idx].to_string(),
            Some(Semver::parse(&s[idx + 1..]).map_err(|e| format!("Version {} provided for package {} invalid: {}", &s[idx + 1..], &s[0..idx], e))?),
            registry_url.map(str::to_string)))
    } else {
        Ok((s.to_string(), None, registry_url.map(str::to_string)))
    }
}

fn duration_parse(s: &str) -> Result<TimeDelta, String> {
    const MULS_S: [char; 6] = ['y', 'w', 'd', 'h', 'm', 's'];
    const MULS_V: [f64; 6] = [365.25 / 7., 7., 24., 60., 60., 1.];
    let (base, mul) = s.strip_suffix(MULS_S).map(|stripped| (stripped, *s.as_bytes().last().unwrap() as _)).unwrap_or((s, 's'));
    let base = f64::from_str(base).map_err(|e| e.to_string())?;
    let val = MULS_V[MULS_S.iter().position(|&c| c == mul).unwrap()..].iter().fold(base, |a, e| a * e);
    let (s, ns) = (val.trunc() as i64, (val.fract() * 1_000_000_000.0) as u32);
    TimeDelta::new(s, ns).ok_or_else(|| format!("{}.{:09} too big", s, ns))
}

#[derive(Copy, Clone)]
struct JobsParser(&'static str, NonZero<usize>);
impl TypedValueParser for JobsParser {
    type Value = NonZero<usize>;
    fn parse_ref(&self, cmd: &Command, arg: Option<&Arg>, value: &OsStr) -> Result<Self::Value, ClapError> {
        let value = value.to_str().ok_or(ClapError::new(ClapErrorKind::InvalidValue).with_cmd(cmd))?;
        let Self(special, default) = *self;

        if value != special {
            if value.starts_with("-") {
                    NonZero::from_str(&value[1..]).map(|sub| if sub >= default {
                        NonZero::new(1).unwrap()
                    } else {
                        NonZero::new(default.get() - sub.get()).unwrap()
                    })
                } else {
                    NonZero::from_str(value)
                }
                .map_err(|e| ClapError::raw(ClapErrorKind::InvalidValue, format_args!("{}: {}", arg.unwrap(), e)).with_cmd(cmd))
        } else {
            Ok(default)
        }
    }
}

#[derive(Copy, Clone)]
struct DefaultFeaturesBoolParser;
impl TypedValueParser for DefaultFeaturesBoolParser {
    type Value = bool;

    fn parse_ref(&self, cmd: &Command, arg: Option<&Arg>, value: &OsStr) -> Result<Self::Value, ClapError> {
        match value.to_str().ok_or(ClapError::new(ClapErrorKind::InvalidValue).with_cmd(cmd))? {
            "1" | "yes" | "true" => Ok(true),
            "0" | "no" | "false" => Ok(false),
            value => {
                Err(ClapError::raw(ClapErrorKind::InvalidValue,
                                   format_args!("{}: {} not 1|yes|true or 0|no|false", arg.unwrap(), value))
                    .with_cmd(cmd))
            }
        }
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
        Some(Box::new(["1", "yes", "true", "0", "no", "false"].iter().map(PossibleValue::new)))
    }
}


fn clerror(f: Arguments) -> ! {
    eprintln!("{}", f);
    exit(1)
}
