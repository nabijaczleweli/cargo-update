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

If that doesn't work and you're on Mac:
 * [re-try with `PKG_CONFIG_PATH=/usr/local/opt/openssl/lib/pkgconfig`](https://github.com/alexcrichton/git2-rs/issues/257),
 * [install OpenSSL via `brew`, and re-try with `LDFLAGS="-L/usr/local/opt/openssl@1.1/lib" CPPFLAGS="-I/usr/local/opt/openssl@1.1/include"`](https://github.com/nabijaczleweli/cargo-update/issues/123),
 * [verify that you don't `openssl` installed twice via `brew`](https://github.com/nabijaczleweli/cargo-update/issues/121#issuecomment-570673813),

If it still doesn't work, [slam open an issue](https://github.com/nabijaczleweli/cargo-update/issues) or [comment on one of the existing relevant ones](https://github.com/nabijaczleweli/cargo-update/issues?q=is%3Aissue+is%3Aopen+label%3Aexternal).

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

### Troubleshooting

Some crates, like `clippy` and `rustfmt`, have moved from Crates.io to being a `rustup` component.
If you'd installed them beforehand, then added them via `rustup component`, they might not have been removed from the list of crates installed via `cargo install`,
  and you [might come across errors](https://github.com/nabijaczleweli/cargo-update/issues/118) such as
```
$ cargo install-update -a
Updating registry 'https://github.com/rust-lang/crates.io-index'

Package          Installed  Latest    Needs update
clippy           v0.0.179   v0.0.302  Yes
.....

Updating clippy
    Updating crates.io index
  Installing clippy v0.0.302
   Compiling clippy v0.0.302
error: failed to compile `clippy v0.0.302`, intermediate artifacts can be found at `/tmp/cargo-installxHfj2y`

Caused by:
  failed to run custom build command for `clippy v0.0.302`

Caused by:
  process didn't exit successfully: `/tmp/cargo-installxHfj2y/release/build/clippy-ffeedc2f188020a4/build-script-build` (exit code: 1)
--- stderr

error: Clippy is no longer available via crates.io

help: please run `rustup component add clippy-preview` instead
```

In that case, run `cargo install --list` to verify that they're still there and `cargo uninstall` them,
  which will deregister the `cargo` versions and leave you with the `rustup` ones.

## Special thanks

To all who support further development on Patreon, in particular:

  * ThePhD
