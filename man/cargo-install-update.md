cargo-install-update(1) -- Cargo subcommand for checking and applying updates to installed executables
======================================================================================================

## SYNOPSIS

`cargo install-update` [OPTIONS] [PACKAGE...]

## DESCRIPTION

Cargo subcommand for checking and applying updates to installed executables.

This was mostly built out of a frustration with periodically checking for
updates for my cargo-installed executables, which was long and boring.

Updates packages from the main repository and git repositories.

See cargo-install-update-config(1) for further configuring updates.

Exit values and possible errors:

    -1 - cargo subprocess was terminated by a signal (Linux-only)
    1  - option parsing error
    2  - registry repository error
    X  - bubbled-up cargo install exit value

## OPTIONS

  -a --all

    Update all currently installed executables.

    Required if list of packages not given.

  [PACKAGE...]

    List of packages to update.

    If specified in addition to --all,
    will add the specified packages to the update list
    (useful, e.g., in conjunction with --allow-no-update).

    Required if --all not given.

  -l --list

    Don't update any packages, just list them.

    If PACKAGE is empty, act as if --all was specified.

  -f --force

    Update all packages, regardless of whether they need to be version-wise.

  -i --allow-no-update

    Allow to fresh install packages passed as PACKAGE argument.

    This is useful, for example, in pairing with cargo-install-update-config(1).

    Off by default.

  -g --git

    Also update git-originating packages.

    Off by default, because it's expensive.

  -s --filter <PACKAGE_FILTER>...

    Only consider packages matching all filters.

    PACKAGE_FILTER is in the form "key=value", where key is any of:
      - "toolchain": the package must be configured to be compiled with
                     the specified toolchain via cargo-install-update-config(1).

  -c --cargo-dir <CARGO_DIR>

    Set the directory containing cargo metadata.

    Required. Default: "$CARGO_HOME", then "$HOME/.cargo", otherwise manual.

  -t --temp-dir <TEMP_DIR>

    Set the directory in which to clone git repositories.

    Adjoined with "cargo-update" as last segment.

    Required. Default: system temp, otherwise manual.

## EXAMPLES

  `cargo install-update -a`

    Update all installed packages.

    Example output:
          Updating registry `https://github.com/rust-lang/crates.io-index`

      Package         Installed  Latest   Needs update
      checksums       v0.5.0     v0.5.2   Yes
      treesize        v0.2.0     v0.2.1   Yes
      cargo-count     v0.2.2     v0.2.2   No
      cargo-graph     v0.3.0     v0.3.0   No
      cargo-outdated  v0.2.0     v0.2.0   No
      identicon       v0.1.1     v0.1.1   No
      racer           v1.2.10    v1.2.10  No
      rustfmt         v0.6.2     v0.6.2   No

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

      Updated 2 packages.

  `cargo install-update` *racer treesize cargo-cln*

    Only consider racer, treesize and cargo-cln for updates.
    Since cargo-cln is not installed, it'll be ignored.

     Example output:
          Updating registry `https://github.com/rust-lang/crates.io-index`

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

      Updated 1 package.

  `cargo install-update -al`

    List all installed packages, don't update any.

    Example output:
          Updating registry `https://github.com/rust-lang/crates.io-index`

      Package         Installed  Latest   Needs update
      checksums       v0.5.0     v0.5.2   Yes
      treesize        v0.2.0     v0.2.1   Yes
      cargo-count     v0.2.2     v0.2.2   No
      cargo-graph     v0.3.0     v0.3.0   No
      cargo-outdated  v0.2.0     v0.2.0   No
      identicon       v0.1.1     v0.1.1   No
      racer           v1.2.10    v1.2.10  No
      rustfmt         v0.6.2     v0.6.2   No

  `cargo install-update -af`

    Update all installed packages.

    Example output:
          Updating registry `https://github.com/rust-lang/crates.io-index`

      Package       Installed  Latest   Needs update
      treesize      v0.2.0     v0.2.1   Yes
      clippy        v0.0.1     v0.0.99  Yes
      clippy_lints  v0.0.1     v0.0.99  Yes
      racer         v1.2.10    v1.2.10  No

      Updating racer
          Updating registry `https://github.com/rust-lang/crates.io-index`
         Downloading racer v1.2.10
         [...]
         Compiling racer v1.2.10
          Finished release [optimized] target(s) in 51.43 secs
         Replacing D:\Users\nabijaczleweli\.cargo\bin\racer.exe

      Updating clippy
          Updating registry `https://github.com/rust-lang/crates.io-index`
         Downloading clippy v0.0.99
         [...]
         Compiling clippy v0.0.99
         [...]
      error: failed to compile `clippy v0.0.99`, intermediate artifacts can be found at `T:\-_-TEM~1\cargo-install.WOcMlrKQ5Sok`

      Updating treesize
          Updating registry `https://github.com/rust-lang/crates.io-index`
         Downloading treesize v0.2.1
         [...]
         Compiling treesize v0.2.1
          Finished release [optimized] target(s) in 76.77 secs
         Replacing D:\Users\nabijaczleweli\.cargo\bin\treesize.exe

      Updating clippy_lints
          Updating registry `https://github.com/rust-lang/crates.io-index`
      error: specified package has no binaries

      Updated 2 packages.
      Failed to update clippy, clippy_lints.

  `cargo install-update -i checksums rustfmt treesize`

    Install specified packages, their installation status notwithstanding

    Example output:
          Updating registry `https://github.com/rust-lang/crates.io-index`

      Package    Installed  Latest   Needs update
      checksums             v0.5.2   Yes
      treesize   v0.2.0     v0.2.1   Yes
      rustfmt    v0.6.2     v0.6.2   No

      Installing checksums
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

      Updated 2 packages.

  `cargo install-update -ag`

    Update all installed packages, including ones from git.

    Example output:
          Updating registry `https://github.com/rust-lang/crates.io-index`

      Package         Installed  Latest   Needs update
      checksums       v0.5.0     v0.5.2   Yes
      cargo-count     v0.2.2     v0.2.2   No

      Updating checksums
          Updating registry `https://github.com/rust-lang/crates.io-index`
         Downloading checksums v0.5.2
         [...]
         Compiling checksums v0.5.2
          Finished release [optimized] target(s) in 95.2 secs
         Replacing D:\Users\nabijaczleweli\.cargo\bin\checksums.exe

      Updated 1 package.

      Package                Installed  Latest   Needs update
      alacritty              eb231b3    5f78857  Yes
      chattium-oxide-client  108a7b9    108a7b9  No

      Updating alacritty from https://github.com/jwilm/alacritty
          Updating git repository `https://github.com/jwilm/alacritty`
         Installing alacritty v0.1.0 (https://github.com/jwilm/alacritty#5f788574)
         [...]
         Compiling alacritty v0.1.0
          Finished release [optimized] target(s) in 127.6 secs
         Replacing D:\Users\nabijaczleweli\.cargo\bin\alacritty.exe

      Updated 1 package.

## AUTHOR

Written by nabijaczleweli &lt;<nabijaczleweli@gmail.com>&gt;,
           Yann Simon &lt;<yann.simon.fr@gmail.com>&gt;,
           ven &lt;<vendethiel@hotmail.fr>&gt;,
           Cat Plus Plus &lt;<piotrlegnica@piotrl.pl>&gt;,
           Liigo &lt;<liigo@qq.com>&gt;,
           azyobuzin &lt;<azyobuzin@users.sourceforge.jp>&gt;,
           Tatsuyuki Ishi &lt;<ishitatsuyuki@gmail.com>&gt;,
           Tom Prince &lt;<tom.prince@twistedmatrix.com>&gt;,
           Mateusz Mikuła &lt;<mati865@gmail.com>&gt;,
           sinkuu &lt;<sinkuupump@gmail.com>&gt;
           Alex Burka &lt;<aburka@seas.upenn.edu>&gt;
           Matthias Krüger &lt;<matthias.krueger@famsik.de>&gt;
       and Daniel Holbert &lt;<dholbert@cs.stanford.edu>&gt;

## SPECIAL THANKS

To all who support further development, in particular:

  * ThePhD

## REPORTING BUGS

&lt;<https://github.com/nabijaczleweli/cargo-update/issues>&gt;

## SEE ALSO

&lt;<https://github.com/nabijaczleweli/cargo-update>&gt;
