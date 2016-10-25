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


use clap::{AppSettings, App, Arg};
use array_tool::vec::Uniq;


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// Packages to update. Default: `None`
    ///
    /// If empty - update all.
    pub to_update: Vec<String>,
}

impl Options {
    /// Parse `env`-wide command-line arguments into an `Options` instance
    pub fn parse() -> Options {
        let matches = App::new("cargo-update")
            .settings(&[AppSettings::ColoredHelp, AppSettings::ArgRequiredElseHelp])
            .version(crate_version!())
            .author(crate_authors!())
            .about("A cargo subcommand for checking and applying updates to installed executables")
            .args(&[Arg::from_usage("-a --all 'Update all packages'").conflicts_with("PACKAGE"),
                    Arg::from_usage("<PACKAGE>... 'Packages to update'").conflicts_with("all").empty_values(false).min_values(1)])
            .get_matches();

        Options {
            to_update: if matches.is_present("all") {
                vec![]
            } else {
                let packages: Vec<_> = matches.values_of("PACKAGE").unwrap().map(String::from).collect();
                packages.unique()
            },
        }
    }
}
