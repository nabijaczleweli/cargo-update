# cargo-update [![TravisCI build status](https://travis-ci.org/nabijaczleweli/cargo-update.svg?branch=master)](https://travis-ci.org/nabijaczleweli/cargo-update) [![AppVeyorCI build status](https://ci.appveyor.com/api/projects/status/cspjknvfow5gfro0/branch/master?svg=true)](https://ci.appveyor.com/project/nabijaczleweli/cargo-update/branch/master) [![Licence](https://img.shields.io/badge/license-MIT-blue.svg?style=flat)](LICENSE) [![Crates.io version](http://meritbadge.herokuapp.com/cargo-update)](https://crates.io/crates/cargo-update)
A [`cargo` subcommand](https://github.com/rust-lang/cargo/wiki/Third-party-cargo-subcommands) for checking and applying updates to installed executables

## [Documentation](https://cdn.rawgit.com/nabijaczleweli/cargo-update/doc/cargo_update/index.html)
## [Manpage](https://cdn.rawgit.com/nabijaczleweli/cargo-update/man/cargo-install-update.1.html)

### Installation

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

### Auto-elevation (and failures thereof) on Windows

For legacy compatibility reasons Windows will try to elevate `cargo-install-update.exe` which will result in something akin to:

```
C:\Users\liigo>cargo install-update
error: An unknown error occurred

To learn more, run the command again with --verbose.

C:\Users\liigo>cargo install-update --verbose
error: An unknown error occurred

To learn more, run the command again with --verbose.
```

That's a known issue, but it's *unresolvable* out of the box until [RFC721](https://github.com/rust-lang/rfcs/issues/721) is accepted.

However, one can override the heuristics by creating a file named `cargo-install-update.exe.manifest` next to the binary itself with the content:

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
    <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
        <security>
            <requestedPrivileges>
                <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
            </requestedPrivileges>
        </security>
    </trustInfo>
</assembly>
```

That way Windows will no longer try to elevate the executable, which will result in it working flawlessly,

See [#11](https://github.com/nabijaczleweli/cargo-update/issues/11) for discussion.
