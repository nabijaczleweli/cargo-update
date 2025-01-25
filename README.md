# cargo-update [![AppVeyorCI build status](https://ci.appveyor.com/api/projects/status/cspjknvfow5gfro0/branch/master?svg=true)](https://ci.appveyor.com/project/nabijaczleweli/cargo-update/branch/master) [![Licence](https://img.shields.io/badge/license-MIT-blue.svg?style=flat)](LICENSE) [![Crates.io version](https://img.shields.io/crates/v/cargo-update)](https://crates.io/crates/cargo-update)
A [`cargo` subcommand](https://github.com/rust-lang/cargo/wiki/Third-party-cargo-subcommands) for checking and applying updates to installed executables

## [Documentation](https://rawcdn.githack.com/nabijaczleweli/cargo-update/doc/cargo_update/index.html)
## [Manual](https://rawcdn.githack.com/nabijaczleweli/cargo-update/man/cargo-install-update.1.html)

### Installation

Firstly, ensure you have [CMake](https://cmake.org) and the [Required Libraries™](#dependencies).

Then proceed as usual:

```shell
cargo install cargo-update
```

If that doesn't work:
 * [re-try with `PKG_CONFIG_PATH=/usr/local/opt/openssl/lib/pkgconfig`](https://github.com/rust-lang/git2-rs/issues/257),
 * [re-try with `LIBSSH2_SYS_USE_PKG_CONFIG=whatever`](https://github.com/nabijaczleweli/cargo-update/issues/129#issuecomment-599269219),
 * [install OpenSSL via `brew`, and re-try with `LDFLAGS="-L/usr/local/opt/openssl@1.1/lib" CPPFLAGS="-I/usr/local/opt/openssl@1.1/include"`](https://github.com/nabijaczleweli/cargo-update/issues/123),
 * [verify that you don't `openssl` installed twice via `brew`](https://github.com/nabijaczleweli/cargo-update/issues/121#issuecomment-570673813),
 * [re-try with `--features vendored-openssl`](https://docs.rs/openssl/0.10.30/openssl/#building),
 * re-try with `--features vendored-libgit2`.
 * re-try with `--features vendored-libcurl`.

If it still doesn't work, [slam open an issue](https://github.com/nabijaczleweli/cargo-update/issues) or [comment on one of the existing relevant ones](https://github.com/nabijaczleweli/cargo-update/issues?q=is%3Aissue+is%3Aopen+label%3Aexternal).

#### Dependencies

| Dependency                                       | Debian package   | Fedora package   | MSYS2 package                 |
|--------------------------------------------------|------------------|------------------|-------------------------------|
| [`libgit2`](https://libgit2.github.com)          | `libgit2-dev`    | `libgit2-devel`  | `mingw-w64-x86_64-libgit2`    |
| [`libcurl`](https://curl.se/libcurl/)            | `libcurl4-*-dev` | `libcurl-devel ` | `libcurl-devel`               |
| [`libssh2`](https://libssh2.org)                 | `libssh-dev`     | `libssh2-devel`  | `mingw-w64-x86_64-libssh2`    |
| [`openssl`](https://openssl.org)                 | `libssl-dev`     | `openssl-devel`  | `mingw-w64-x86_64-openssl`    |
| [`pkgconf`](http://pkgconf.org) (some platforms) | `pkgconf`        | `pkgconf`        | `mingw-w64-x86_64-pkgconf`    |

### Usage

`cargo install-update -a` — check for newer versions and update all installed packages.

`cargo install-update crate1 crate2 ...` — check for newer versions and update selected packages, will not install new packages.

For more information and examples see the [manual](https://rawcdn.githack.com/nabijaczleweli/cargo-update/man/cargo-install-update.1.html).

`cargo install-update-config -t unstable -f feature1 -d false crate` — when building crate, do so with the unstable toolchain with feature1 and no default features.

For more information and examples see the [manual](https://rawcdn.githack.com/nabijaczleweli/cargo-update/man/cargo-install-update-config.1.html).

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

### Bleeding-edge `cargo`s

Since [`0.42.0`](https://github.com/rust-lang/cargo/commit/fb4415090f600bae51b0747bef2e7049070cd6ee),
  `cargo install cratename` checks for newer versions and installs them if they exist, instead of erroring out like it does usually.

### Source Replacement vs custom registries

Cargo allows [replacing entire registries at a time](https://doc.rust-lang.org/cargo/reference/source-replacement.html).

For example, this stanza in `~/.cargo/config` will replace the default crates.io registry with the Shanghai Jiao Tong Universty's mirror:
```toml
[source.crates-io]
replace-with = "sjtu"

[source.sjtu]
registry = "https://mirrors.sjtug.sjtu.edu.cn/git/crates.io-index"
```

`cargo-update` resolves this to the deepest registry, and passes `--registry sjtu` to `cargo install`.
This worked until roughly `nightly-2019-08-10`, but since `nightly-2019-09-10` due to a Cargo regression (or feature, but it's breaking without a major version bump, so)
`--registry` looks into a different key, requiring this additional stanza to ensure correct updates:
```toml
[registries.sjtu]
index = "https://mirrors.sjtug.sjtu.edu.cn/git/crates.io-index"
```

Confer the [initial implementation](https://github.com/nabijaczleweli/cargo-update/issues/107), [rewrite](https://github.com/nabijaczleweli/cargo-update/issues/128),
[final broken testcase](https://github.com/nabijaczleweli/cargo-update/issues/137) and
[final debug implementation](https://github.com/nabijaczleweli/cargo-update/pull/138) threads
(h/t [@DCJanus](https://github.com/DCjanus) for help debugging and testcases, also
 dealing with me as I slowly [spiraled](https://lfs.nabijaczleweli.xyz/0017-twitter-export#1288559898763157511) into insanity).

## Special thanks

To all who support further development on Patreon, in particular:

  * ThePhD
  * Embark Studios
  * Lars Strojny
  * EvModder
