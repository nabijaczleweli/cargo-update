//! A [`cargo` subcommand](https://github.com/rust-lang/cargo/wiki/Third-party-cargo-subcommands) for checking and applying
//! updates to installed executables
//!
//! # Special thanks
//!
//! To all who support further development on [Patreon](https://patreon.com/nabijaczleweli), in particular:
//!
//!   * ThePhD
//!   * Embark Studios
//!   * Lars Strojny
//!   * EvModder
//!
//! # Library doc
//!
//! This library is used by `cargo-update` itself for all its function and is therefore contains all necessary functions.
//!
//! ## Example
//!
//! See the [`src/main.rs`](https://github.com/nabijaczleweli/cargo-update/blob/master/src/main.rs) file in the git repository.
//!
//! # Executable manpage
//!
//! ## SYNOPSIS
//!
//! [`cargo install-update`](https://github.com/nabijaczleweli/cargo-update) [OPTIONS] [PACKAGE...]
//!
//! ## DESCRIPTION
//!
//!
//! Cargo subcommand for checking and applying updates to installed executables.
//!
//! This was mostly built out of a frustration with periodically checking for
//! updates for my cargo-installed executables, which was long and boring.
//!
//! Updates packages from the main repository and git repositories.
//!
//! See
//! [cargo-install-update-config(1)](https://rawcdn.githack.com/nabijaczleweli/cargo-update/man/cargo-install-update-config.1.html)
//! for further configuring updates.
//!
//! The `CARGO_INSTALL_OPTS` environment variable can be set,
//! containing options to forward to the end of `cargo install` invocations'
//! argument lists.
//! Note, that
//! [cargo-install-update-config(1)](https://rawcdn.githack.com/nabijaczleweli/cargo-update/man/cargo-install-update-config.1.html)
//! is preferred in the general case.
//!
//! Exit values and possible errors:
//!
//! ```text
//! -1 - cargo subprocess was terminated by a signal (Linux-only)
//! 1  - option parsing error
//! 2  - registry repository error
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
//! Required if list of packages not given.
//! ```
//!
//! [PACKAGE...]
//!
//! ```text
//! List of packages to update in the [(registry_url):]package_name[:version] format.
//!
//! Registry defaults to the default crates.io registry,
//! and can be a name from ~/.cargo/config.
//!
//! If specified in addition to --all,
//! will add the specified packages to the update list
//! (useful, e.g., in conjunction with --allow-no-update).
//!
//! Required if --all not given.
//! ```
//!
//! -l --list
//!
//! ```text
//! Don't update any packages, just list them.
//!
//! If PACKAGE is empty, act as if --all was specified.
//! ```
//!
//! -f --force
//!
//! ```text
//! Update all packages, regardless of whether they need to be version-wise.
//! ```
//!
//! -i --allow-no-update
//!
//! ```text
//! Allow to fresh install packages passed as PACKAGE argument.
//!
//! This is useful, for example, in pairing with cargo-install-update-config(1).
//!
//! Off by default.
//! ```
//!
//! -g --git
//!
//! ```text
//! Also update git-originating packages.
//!
//! Off by default, because it's expensive.
//! ```
//!
//! -q --quiet
//!
//! ```text
//! Don't print status messages to stdout
//! and pass down --quiet to cargo subprocesses.
//! ```
//!
//! -s --filter <PACKAGE_FILTER>...
//!
//! ```text
//! Only consider packages matching all filters.
//!
//! PACKAGE_FILTER is in the form "key=value", where key is any of:
//!   - "toolchain": the package must be configured to be compiled with
//!                  the specified toolchain via cargo-install-update-config(1).
//! ```
//!
//! -c --cargo-dir <CARGO_DIR>
//!
//! ```text
//! Set the directory containing cargo metadata.
//!
//! Required. Default: "$CARGO_HOME", then "$HOME/.cargo", otherwise manual.
//! ```
//!
//! -t --temp-dir <TEMP_DIR>
//!
//! ```text
//! Set the directory in which to clone git repositories.
//!
//! Adjoined with "cargo-update" as last segment.
//!
//! Required. Default: system temp, otherwise manual.
//! ```
//!
//! ## ENVIRONMENT VARIABLES
//!
//! ##### CARGO_REGISTRIES_CRATES_IO_PROTOCOL
//!
//! Overrides the `registries.crates-io.protocol` Cargo configuration key.
//!
//! The default is `sparse`, and the crates.io URL is
//! sparse+https://index.crates.io/.
//! Set to some other value to use the git registry
//! (https://github.com/rust-lang/crates.io-index) for crates.io.
//!
//! ##### CARGO_NET_GIT_FETCH_WITH_CLI
//!
//! Overrides the `net.git-fetch-with-cli` Cargo configuration key.
//!
//! ##### GIT
//!
//! Overrides the git executable in `net.git-fetch-with-cli=true` mode.
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
//!   checksums       v0.5.0     v0.5.2   Yes
//!   treesize        v0.2.0     v0.2.1   Yes
//!   cargo-count     v0.2.2     v0.2.2   No
//!   cargo-graph     v0.3.0     v0.3.0   No
//!   cargo-outdated  v0.2.0     v0.2.0   No
//!   identicon       v0.1.1     v0.1.1   No
//!   racer           v1.2.10    v1.2.10  No
//!   rustfmt         v0.6.2     v0.6.2   No
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
//!   treesize  v0.2.0     v0.2.1   Yes
//!   racer     v1.2.10    v1.2.10  No
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
//!   checksums       v0.5.0     v0.5.2   Yes
//!   treesize        v0.2.0     v0.2.1   Yes
//!   cargo-count     v0.2.2     v0.2.2   No
//!   cargo-graph     v0.3.0     v0.3.0   No
//!   cargo-outdated  v0.2.0     v0.2.0   No
//!   identicon       v0.1.1     v0.1.1   No
//!   racer           v1.2.10    v1.2.10  No
//!   rustfmt         v0.6.2     v0.6.2   No
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
//!   treesize  v0.2.0     v0.2.1   Yes
//!   racer     v1.2.10    v1.2.10  No
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
//!
//! `cargo install-update -i checksums rustfmt treesize`
//!
//! ```text
//! Install specified packages, their installation status notwithstanding
//!
//! Example output:
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!
//!   Package    Installed  Latest   Needs update
//!   checksums             v0.5.2   Yes
//!   treesize   v0.2.0     v0.2.1   Yes
//!   rustfmt    v0.6.2     v0.6.2   No
//!
//!   Installing checksums
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
//! `cargo install-update -i (file:///usr/local/share/cargo):zram-generator:0.1.1`
//!
//! ```text
//! Install zram-generator from a local repository in /usr/local/share/cargo
//! (but a remote one or a short name will work just as well), at most version 0.1.1.
//!
//!  Example output:
//!       Updating registry `file:///usr/local/share/cargo`
//!
//!   Package         Installed  Latest   Needs update
//!   zram-generator             v0.1.1   Yes
//!
//!   Installing zram-generator
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!      Downloading zram-generator v0.1.1
//!      [...]
//!      Compiling zram-generator v0.1.1
//!       Finished release [optimized] target(s) in 21.62 secs
//!     Installing /home/nabijaczleweli/.cargo/bin/zram-generator
//!      Installed package `zram-generator v0.1.1` (executable `zram-generator`)
//!
//!   Updated 1 package.
//! ```
//!
//! `cargo install-update -ag`
//!
//! ```text
//! Update all installed packages, including ones from git.
//!
//! Example output:
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!
//!   Package         Installed  Latest   Needs update
//!   checksums       v0.5.0     v0.5.2   Yes
//!   cargo-count     v0.2.2     v0.2.2   No
//!
//!   Updating checksums
//!       Updating registry `https://github.com/rust-lang/crates.io-index`
//!      Downloading checksums v0.5.2
//!      [...]
//!      Compiling checksums v0.5.2
//!       Finished release [optimized] target(s) in 95.2 secs
//!      Replacing D:\Users\nabijaczleweli\.cargo\bin\checksums.exe
//!
//!   Updated 1 package.
//!
//!   Package                Installed  Latest   Needs update
//!   alacritty              eb231b3    5f78857  Yes
//!   chattium-oxide-client  108a7b9    108a7b9  No
//!
//!   Updating alacritty from https://github.com/jwilm/alacritty
//!       Updating git repository `https://github.com/jwilm/alacritty`
//!      Installing alacritty v0.1.0 (https://github.com/jwilm/alacritty#5f788574)
//!      [...]
//!      Compiling alacritty v0.1.0
//!       Finished release [optimized] target(s) in 127.6 secs
//!      Replacing D:\Users\nabijaczleweli\.cargo\bin\alacritty.exe
//!
//!   Updated 1 package.
//! ```


#![cfg_attr(feature = "cargo-clippy", allow(redundant_field_names))]


extern crate json_deserializer;
#[macro_use]
extern crate serde_derive;
extern crate array_tool;
extern crate once_cell;
extern crate semver;
extern crate regex;
extern crate git2;
#[macro_use]
extern crate clap;
extern crate curl;
extern crate home;
extern crate toml;
extern crate hex;
extern crate url;

mod options;

pub mod ops;

pub use options::{ConfigOptions, Options};
