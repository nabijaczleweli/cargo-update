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


use clap::{self, AppSettings, SubCommand, App, Arg};
use self::super::ops::ConfigOperation;
use std::env::{self, home_dir};
use array_tool::vec::Uniq;
use std::path::PathBuf;
use std::fs;


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// Packages to update. Default: `None`
    ///
    /// If empty - update all.
    pub to_update: Vec<String>,
    /// Whether to update packages or just list them. Default: `true`
    pub update: bool,
    /// Whether to allow for just installing packages. Default: `false`
    pub install: bool,
    /// Update all packages. Default: `false`
    pub force: bool,
    /// The `cargo` home directory. Default: in `"$CARGO_INSTALL_ROOT"`, then `"$CARGO_HOME"`, then `"$HOME/.cargo"`
    pub crates_file: (String, PathBuf),
    /// The `cargo` home directory. Default: `"$CARGO_HOME"`, then `"$HOME/.cargo"`
    pub cargo_dir: (String, PathBuf),
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
        let matches = App::new("cargo-install-update")
            .bin_name("cargo")
            .settings(&[AppSettings::ColoredHelp, AppSettings::ArgRequiredElseHelp, AppSettings::GlobalVersion, AppSettings::SubcommandRequired])
            .subcommand(SubCommand::with_name("install-update")
                .version(crate_version!())
                .author(crate_authors!("\n"))
                .about("A cargo subcommand for checking and applying updates to installed executables")
                .args(&[Arg::from_usage("-c --cargo-dir=[CARGO_DIR] 'The cargo home directory. Default: $CARGO_HOME or $HOME/.cargo'")
                            .validator(cargo_dir_validator),
                        Arg::from_usage("-a --all 'Update all packages'").conflicts_with("PACKAGE"),
                        Arg::from_usage("-l --list 'Don't update packages, only list and check if they need an update'"),
                        Arg::from_usage("-f --force 'Update all packages regardless if they need updating'"),
                        Arg::from_usage("--allow-no-update 'Allow for fresh-installing packages'"),
                        Arg::from_usage("<PACKAGE>... 'Packages to update'").conflicts_with("all").empty_values(false).min_values(1)]))
            .get_matches();
        let matches = matches.subcommand_matches("install-update").unwrap();

        let cdir = cargo_dir();
        Options {
            to_update: if matches.is_present("all") {
                vec![]
            } else {
                let packages: Vec<_> = matches.values_of("PACKAGE").unwrap().map(String::from).collect();
                packages.unique()
            },
            update: !matches.is_present("list"),
            install: matches.is_present("allow-no-update"),
            force: matches.is_present("force"),
            crates_file: match matches.value_of("cargo-dir") {
                Some(dirs) => (dirs.to_string(), fs::canonicalize(dirs).unwrap()),
                None => {
                    match env::var("CARGO_INSTALL_ROOT").map_err(|_| ()).and_then(|ch| fs::canonicalize(ch).map_err(|_| ())) {
                        Ok(ch) => ("$CARGO_INSTALL_ROOT/.crates.toml".to_string(), ch.join(".crates.toml")),
                        Err(()) => (format!("{}/.crates.toml", cdir.0), cdir.1.join(".crates.toml")),
                    }
                }
            },
            cargo_dir: cdir,
        }
    }
}

impl ConfigOptions {
    /// Parse `env`-wide command-line arguments into an `ConfigOptions` instance
    pub fn parse() -> ConfigOptions {
        let matches = App::new("cargo-install-update-config")
            .bin_name("cargo")
            .settings(&[AppSettings::ColoredHelp, AppSettings::ArgRequiredElseHelp, AppSettings::GlobalVersion, AppSettings::SubcommandRequired])
            .subcommand(SubCommand::with_name("install-update-config")
                .version(crate_version!())
                .author(crate_authors!("\n"))
                .about("A cargo subcommand for checking and applying updates to installed executables -- configuration")
                .args(&[Arg::from_usage("-c --cargo-dir=[CARGO_DIR] 'The cargo home directory. Default: $CARGO_HOME or $HOME/.cargo'")
                            .validator(cargo_dir_validator),
                        Arg::from_usage("-t --toolchain=[TOOLCHAIN] 'Toolchain to use or empty for default'"),
                        Arg::from_usage("-f --feature=[FEATURE]... 'Feature to enable'"),
                        Arg::from_usage("-n --no-feature=[DISABLED_FEATURE]... 'Feature to disable'"),
                        Arg::from_usage("-d --default-features=[DEFAULT_FEATURES] 'Whether to allow default features'")
                            .possible_values(&["1", "yes", "true", "0", "no", "false"])
                            .hide_possible_values(true),
                        Arg::from_usage("<PACKAGE> 'Packages to update'").empty_values(false)]))
            .get_matches();
        let matches = matches.subcommand_matches("install-update-config").unwrap();

        let cdir = cargo_dir();
        ConfigOptions {
            crates_file: match matches.value_of("cargo-dir") {
                Some(dirs) => (dirs.to_string(), fs::canonicalize(dirs).unwrap()),
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

fn cargo_dir_validator(s: String) -> Result<(), String> {
    fs::canonicalize(&s).map(|_| ()).map_err(|_| format!("Cargo directory \"{}\" not found", s))
}
