# cargo-update [![TravisCI build status](https://travis-ci.org/nabijaczleweli/cargo-update.svg?branch=master)](https://travis-ci.org/nabijaczleweli/cargo-update) [![AppVeyorCI build status](https://ci.appveyor.com/api/projects/status/cspjknvfow5gfro0/branch/master?svg=true)](https://ci.appveyor.com/project/nabijaczleweli/cargo-update/branch/master) [![Licence](https://img.shields.io/badge/license-MIT-blue.svg?style=flat)](LICENSE) [![Crates.io version](http://meritbadge.herokuapp.com/cargo-update)](https://crates.io/crates/cargo-update)
A [`cargo` subcommand](https://github.com/rust-lang/cargo/wiki/Third-party-cargo-subcommands) for checking and applying updates to installed executables

## [Documentation](https://cdn.rawgit.com/nabijaczleweli/cargo-update/doc/cargo_update/index.html)
## [Manpage](https://cdn.rawgit.com/nabijaczleweli/cargo-update/man/cargo-install-update.1.html)

### Installation

Make sure that `cmake` is installed on your system. Example for installing it on Ubuntu:

```shell
sudo apt install cmake
```

Then install cargo-update with cargo:

```shell
cargo install cargo-update
```

### Usage

`cargo install-update -a` - check for newer versions and update all installed packages.

`cargo install-update crate1 crate2 ...` - check for newer versions and update selected packages, will not install new packages.

For more information and examples see the [manpage](https://cdn.rawgit.com/nabijaczleweli/cargo-update/man/cargo-install-update.1.html).

#### Self-update

`cargo-update` will update itself seamlessly on Linux and Windows.

On Windows the following strategy is applied:
  * Check for old versions, remove them
  * Add the current version to the current executable's extension
  * Create an empty file in place of the just-renamed file (this way `cargo install` will "replace" it and not duplicate the entry in `.crates.toml`)
