//! A [`cargo` subcommand](https://github.com/rust-lang/cargo/wiki/Third-party-cargo-subcommands) for checking and applying updates to installed executables
//!
//! # Library doc
//!
//! This library is used by `cargo-update` itself for all its function and is therefore contains all necessary functions.
//!
//! ## Data flow
//!
//! ```text
//! Options::parse()
//! |> installed_main_repo_packages()
//! |> intersect_packages()
//! |> get_index_path()
//! |> MainRepoPackage::pull_version()
//! ```
//!
//! # Executable manpage
//!
//! ## SYNOPSIS
//!
//! [`cargo install-update`](https://github.com/nabijaczleweli/cargo-update) [OPTIONS] [PACKAGE...]
//!
//! ## DESCRIPTION
//!
//! Cargo subcommand for checking and applying updates to installed executables.
//!
//! This was mostly built out of a frustration with periodically checking for
//! updates for my cargo-installed executables, which was long and boring.
//!
//! Only updates packages from the main repository.
//!
//! Exit values and possible errors:
//!
//! ```text
//! -1 - cargo subprocess was terminated by a signal (Linux-only)
//! 1  - option parsing error
//! X  - bubbled-up cargo install exit value
//! ```
//!
//! ## OPTIONS
//!
//! -a --all
//!
//! ```text
//! Update all currently installed executables.
//!
//! Exclusive with list of packages. Required if list of packages not given.
//! ```
//!
//! [PACKAGE...]
//!
//! ```text
//! List of packages to update.
//!
//! Exclusive with --all. Required if --all not given.
//! ```
//!
//! -l --list
//!
//! ```text
//! Don't update any packages, just list them.
//! ```
//!
//! -f --force
//!
//! ```text
//! Update all packages, regardless of whether they need to be version-wise.
//! ```
//!
//! -c --cargo-dir &lt;CARGO_DIR&gt;
//!
//! ```text
//! Set the directory containing cargo metadata.
//!
//! Required. Default: "$CARGO_HOME", then $HOME/.cargo", otherwise manual.
//! ```
//!
//! ## EXAMPLES
//!
//! `cargo install-update -a`
//!
//! ```text
//! Update all installed packages.
//!
//! Example output:
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!
//!   Package         Installed  Latest   Needs update
//!   cargo-count     v0.2.2     v0.2.2   No
//!   cargo-graph     v0.3.0     v0.3.0   No
//!   cargo-outdated  v0.2.0     v0.2.0   No
//!   checksums       v0.5.0     v0.5.2   Yes
//!   identicon       v0.1.1     v0.1.1   No
//!   racer           v1.2.10    v1.2.10  No
//!   rustfmt         v0.6.2     v0.6.2   No
//!   treesize        v0.2.0     v0.2.1   Yes
//!
//!   Updating checksums
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!      Downloading checksums v0.5.2
//!      [...]
//!      Compiling checksums v0.5.2
//!       Finished release [optimized] target(s) in 95.2 secs
//!      Replacing D:\Users\nabijaczleweli\.cargo\bin\checksums.exe
//!
//!   Updating treesize
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!      Downloading treesize v0.2.1
//!      [...]
//!      Compiling treesize v0.2.1
//!       Finished release [optimized] target(s) in 76.77 secs
//!      Replacing D:\Users\nabijaczleweli\.cargo\bin\treesize.exe
//!
//!   Updated 2 packages.
//! ```
//!
//! `cargo install-update` *racer treesize cargo-cln*
//!
//! ```text
//! Only consider racer, treesize and cargo-cln for updates.
//! Since cargo-cln is not installed, it'll be ignored.
//!
//!  Example output:
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!
//!   Package   Installed  Latest   Needs update
//!   racer     v1.2.10    v1.2.10  No
//!   treesize  v0.2.0     v0.2.1   Yes
//!
//!   Updating treesize
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!      Downloading treesize v0.2.1
//!      [...]
//!      Compiling treesize v0.2.1
//!       Finished release [optimized] target(s) in 76.77 secs
//!      Replacing D:\Users\nabijaczleweli\.cargo\bin\treesize.exe
//!
//!   Updated 1 package.
//! ```
//!
//! `cargo install-update -al`
//!
//! ```text
//! List all installed packages, don't update any.
//!
//! Example output:
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!
//!   Package         Installed  Latest   Needs update
//!   cargo-count     v0.2.2     v0.2.2   No
//!   cargo-graph     v0.3.0     v0.3.0   No
//!   cargo-outdated  v0.2.0     v0.2.0   No
//!   checksums       v0.5.0     v0.5.2   Yes
//!   identicon       v0.1.1     v0.1.1   No
//!   racer           v1.2.10    v1.2.10  No
//!   rustfmt         v0.6.2     v0.6.2   No
//!   treesize        v0.2.0     v0.2.1   Yes
//! ```
//!
//! `cargo install-update -af`
//!
//! ```text
//! Update all installed packages.
//! Example output:
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!
//!   Package   Installed  Latest   Needs update
//!   racer     v1.2.10    v1.2.10  No
//!   treesize  v0.2.0     v0.2.1   Yes
//!
//!   Updating racer
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!      Downloading racer v1.2.10
//!      [...]
//!      Compiling racer v1.2.10
//!       Finished release [optimized] target(s) in 51.43 secs
//!      Replacing D:\Users\nabijaczleweli\.cargo\bin\racer.exe
//!
//!   Updating clippy
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!      Downloading clippy v0.0.99
//!      [...]
//!      Compiling clippy v0.0.99
//!      [...]
//!   error: failed to compile `clippy v0.0.99`
//!
//!   Updating treesize
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!      Downloading treesize v0.2.1
//!      [...]
//!      Compiling treesize v0.2.1
//!       Finished release [optimized] target(s) in 76.77 secs
//!      Replacing D:\Users\nabijaczleweli\.cargo\bin\treesize.exe
//!
//!   Updating clippy_lints
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!   error: specified package has no binaries
//!
//!   Updated 2 packages.
//!   Failed to update clippy, clippy_lints.
//! ```


#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
extern crate array_tool;
extern crate semver;
extern crate regex;
#[macro_use]
extern crate clap;
extern crate toml;
extern crate json;

mod options;

pub mod ops;

pub use options::{ConfigOptions, Options};
