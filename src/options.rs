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
    /// Update all packages. Default: `false`
    pub force: bool,
    /// The `cargo` home directory. Default: in `"$CARGO_INSTALL_ROOT"`, then `"$CARGO_HOME"`, then `"$HOME/.cargo"`
    pub crates_file: (String, PathBuf),
    /// The `cargo` home directory. Default: `"$CARGO_HOME"`, then `"$HOME/.cargo"`
    pub cargo_dir: (String, PathBuf),
}

impl Options {
    /// Parse `env`-wide command-line arguments into an `Options` instance
    pub fn parse() -> Options {
        let matches = App::new("cargo-install-update")
            .bin_name("cargo")
            .settings(&[AppSettings::ColoredHelp, AppSettings::ArgRequiredElseHelp, AppSettings::GlobalVersion, AppSettings::SubcommandRequired])
            .subcommand(SubCommand::with_name("install-update")
                .version(crate_version!())
                .author(crate_authors!())
                .about("A cargo subcommand for checking and applying updates to installed executables")
                .args(&[Arg::from_usage("-c --cargo-dir=[CARGO_DIR] 'The cargo home directory. Default: $CARGO_HOME or $HOME/.cargo'")
                            .validator(Options::cargo_dir_validator),
                        Arg::from_usage("-a --all 'Update all packages'").conflicts_with("PACKAGE"),
                        Arg::from_usage("-l --list 'Don't update packages, only list and check if they need an update'"),
                        Arg::from_usage("-f --force 'Update all packages regardless if they need updating'"),
                        Arg::from_usage("<PACKAGE>... 'Packages to update'").conflicts_with("all").empty_values(false).min_values(1)]))
            .get_matches();
        let matches = matches.subcommand_matches("install-update").unwrap();

        let cdir = match env::var("CARGO_HOME").map_err(|_| ()).and_then(|ch| fs::canonicalize(ch).map_err(|_| ())) {
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
        };
        Options {
            to_update: if matches.is_present("all") {
                vec![]
            } else {
                let packages: Vec<_> = matches.values_of("PACKAGE").unwrap().map(String::from).collect();
                packages.unique()
            },
            update: !matches.is_present("list"),
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

    fn cargo_dir_validator(s: String) -> Result<(), String> {
        fs::canonicalize(&s).map(|_| ()).map_err(|_| format!("Cargo directory \"{}\" not found", s))
    }
}
