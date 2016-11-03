cargo-install-update(1) -- Cargo subcommand for checking and applying updates to installed executables
======================================================================================================

## SYNOPSIS

`cargo install-update` [OPTIONS] [PACKAGE...]

## DESCRIPTION

Cargo subcommand for checking and applying updates to installed executables.

This was mostly built out of a frustration with periodically checking for
updates for my cargo-installed executables, which was long and boring.

Only updates packages from the main repository.

Exit values and possible errors:

    -1 - cargo subprocess was terminated by a signal (Linux-only)
    1  - option parsing error
    X  - bubbled-up cargo install exit value

## OPTIONS

  -a --all

    Update all currently installed executables.

    Exclusive with list of packages. Required if list of packages not given.

  [PACKAGE...]

    List of packages to update.

    Exclusive with --all. Required if --all not given.

  -l --list

    Don't update any packages, just list them.

  -c --cargo-dir <CARGO_DIR>

    Set the directory containing cargo metadata.

    Required. Default: "$CARGO_HOME", then $HOME/.cargo", otherwise manual.

## EXAMPLES

  `cargo install-update -a`

    Update all installed packages.

    Example output:
      Package         Installed  Latest   Needs update
      cargo-count     v0.2.2     v0.2.2   No
      cargo-graph     v0.3.0     v0.3.0   No
      cargo-outdated  v0.2.0     v0.2.0   No
      checksums       v0.5.0     v0.5.2   Yes
      identicon       v0.1.1     v0.1.1   No
      racer           v1.2.10    v1.2.10  No
      rustfmt         v0.6.2     v0.6.2   No
      treesize        v0.2.0     v0.2.1   Yes

      Updating checksums
          Updating registry `https://github.com/rust-lang/crates.io-index`
         Downloading checksums v0.5.2
         [...]
         Compiling checksums v0.5.2
          Finished release [optimized] target(s) in 95.2 secs
         Replacing D:\Users\nabijaczleweli\.cargo\bin\checksums.exe

      Updating treesize
          Updating registry `https://github.com/rust-lang/crates.io-index`
         Downloading treesize v0.2.1
         [...]
         Compiling treesize v0.2.1
          Finished release [optimized] target(s) in 76.77 secs
         Replacing D:\Users\nabijaczleweli\.cargo\bin\treesize.exe

  `cargo install-update` *racer treesize cargo-cln*

    Only consider racer, treesize and cargo-cln for updates.
    Since cargo-cln is not installed, it'll be ignored.

     Example output:
      Package   Installed  Latest   Needs update
      racer     v1.2.10    v1.2.10  No
      treesize  v0.2.0     v0.2.1   Yes

      Updating treesize
          Updating registry `https://github.com/rust-lang/crates.io-index`
         Downloading treesize v0.2.1
         [...]
         Compiling treesize v0.2.1
          Finished release [optimized] target(s) in 76.77 secs
         Replacing D:\Users\nabijaczleweli\.cargo\bin\treesize.exe

  `cargo install-update -al`

    List all installed packages, don't update any.

    Example output:
      Package         Installed  Latest   Needs update
      cargo-count     v0.2.2     v0.2.2   No
      cargo-graph     v0.3.0     v0.3.0   No
      cargo-outdated  v0.2.0     v0.2.0   No
      checksums       v0.5.0     v0.5.2   Yes
      identicon       v0.1.1     v0.1.1   No
      racer           v1.2.10    v1.2.10  No
      rustfmt         v0.6.2     v0.6.2   No
      treesize        v0.2.0     v0.2.1   Yes

## AUTHOR

Written by nabijaczleweli &lt;<nabijaczleweli@gmail.com>&gt;
       and Yann Simon &lt;<yann.simon.fr@gmail.com>&gt;

## REPORTING BUGS

&lt;<https://github.com/nabijaczleweli/cargo-update/issues>&gt;

## SEE ALSO

&lt;<https://github.com/nabijaczleweli/cargo-update>&gt;
