# cargo-update [![TravisCI build status](https://travis-ci.org/nabijaczleweli/cargo-update.svg?branch=master)](https://travis-ci.org/nabijaczleweli/cargo-update) [![AppVeyorCI build status](https://ci.appveyor.com/api/projects/status/cspjknvfow5gfro0/branch/master?svg=true)](https://ci.appveyor.com/project/nabijaczleweli/cargo-update/branch/master) [![Licence](https://img.shields.io/badge/license-MIT-blue.svg?style=flat)](LICENSE) [![Crates.io version](https://meritbadge.herokuapp.com/cargo-update)](https://crates.io/crates/cargo-update)
A [`cargo` subcommand](https://github.com/rust-lang/cargo/wiki/Third-party-cargo-subcommands) for checking and applying updates to installed executables

## [Documentation](https://rawcdn.githack.com/nabijaczleweli/cargo-update/doc/cargo_update/index.html)
## [Manpage](https://rawcdn.githack.com/nabijaczleweli/cargo-update/man/cargo-install-update.1.html)

### Installation

Firstly, ensure you have [CMake](https://cmake.org) and the [Required Libraries™](#required-libraries).

Then proceed as usual:

```shell
cargo install cargo-update
```

If that doesn't work and you're on Mac, [re-try with `PKG_CONFIG_PATH=/usr/local/opt/openssl/lib/pkgconfig`](https://github.com/alexcrichton/git2-rs/issues/257). If it still doesn't work, [slam open an issue](https://github.com/nabijaczleweli/cargo-update/issues) or [comment on one of the existing relevant ones](https://github.com/nabijaczleweli/cargo-update/issues?q=is%3Aissue+is%3Aopen+label%3Aexternal).

#### Required libraries

| Library                                 | \*X package name | msys2 package name         |
|-----------------------------------------|------------------|----------------------------|
| [`libgit2`](https://libgit2.github.com) | `libgit2-devel`  | `mingw-w64-x86_64-libgit2` |
| [`libssh2`](https://libssh2.org)        | `libssh2-devel`  | `mingw-w64-x86_64-libssh2` |
| [`openssl`](https://openssl.org)        | `openssl-devel`  | `mingw-w64-x86_64-openssl` |

### Usage

`cargo install-update -a` - check for newer versions and update all installed packages.

`cargo install-update crate1 crate2 ...` - check for newer versions and update selected packages, will not install new packages.

For more information and examples see the [manpage](https://rawcdn.githack.com/nabijaczleweli/cargo-update/man/cargo-install-update.1.html).

#### Self-update

`cargo-update` will update itself seamlessly on Linux and Windows.

On Windows the following strategy is applied:
  * Check for old versions, remove them
  * Add the current version to the current executable's extension
  * Create an empty file in place of the just-renamed file (this way `cargo install` will "replace" it and not duplicate the entry in `.crates.toml`)

## Special thanks

To all who support further development on Patreon, in particular:

  * ThePhD
