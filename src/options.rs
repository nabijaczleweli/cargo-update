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
use array_tool::vec::Uniq;
use std::path::PathBuf;
use std::env::home_dir;
use std::fs;


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// Packages to update. Default: `None`
    ///
    /// If empty - update all.
    pub to_update: Vec<String>,
    /// The `cargo` home directory. Default: `"$HOME/.cargo"`
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
                .args(&[Arg::from_usage("-c --cargo-dir=[CARGO_DIR] 'The cargo home directory. Default: $HOME/.cargo'")
                            .validator(Options::cargo_dir_validator),
                        Arg::from_usage("-a --all 'Update all packages'").conflicts_with("PACKAGE"),
                        Arg::from_usage("<PACKAGE>... 'Packages to update'").conflicts_with("all").empty_values(false).min_values(1)]))
            .get_matches();
        let matches = matches.subcommand_matches("install-update").unwrap();

        Options {
            to_update: if matches.is_present("all") {
                vec![]
            } else {
                let packages: Vec<_> = matches.values_of("PACKAGE").unwrap().map(String::from).collect();
                packages.unique()
            },
            cargo_dir: match matches.value_of("cargo-dir") {
                Some(dirs) => (dirs.to_string(), fs::canonicalize(dirs).unwrap()),
                None => {
                    match home_dir() {
                        Some(mut hd) => {
                            hd = hd.canonicalize().unwrap();
                            hd.push(".cargo");

                            fs::create_dir_all(&hd).unwrap();
                            ("$HOME/.cargo".to_string(), hd)
                        }
                        None => {
                            clap::Error {
                                    message: "Couldn't automatically get home directory, please specify the cargo home directory with the -c option"
                                        .to_string(),
                                    kind: clap::ErrorKind::MissingRequiredArgument,
                                    info: None,
                                }
                                .exit()
                        }
                    }
                }
            },
        }
    }

    fn cargo_dir_validator(s: String) -> Result<(), String> {
        fs::canonicalize(&s).map(|_| ()).map_err(|_| format!("Cargo directory \"{}\" not found", s))
    }
}
