cargo-install-update(1) -- Cargo subcommand for checking and applying updates to installed executables
======================================================================================================

## SYNOPSIS

`cargo install-update` [OPTIONS] [PACKAGE...]

## DESCRIPTION

Cargo subcommand for checking and applying updates to installed executables.

This was mostly built out of a frustration with periodically checking for
updates for my cargo-installed executables, which was long and boring.

Updates packages from the main repository and git repositories.

See cargo-install-update-config(1) for further configuring updates,
and the metadata from `cargo install` that may be preserved by default.

The `CARGO_INSTALL_OPTS` environment variable can be set,
containing options to forward to the end of `cargo install` invocations'
argument lists.
Note, that cargo-install-update-config(1) is preferred in the general case.

If `cargo-binstall` (>=0.13.1) is available in the `PATH`,
`-r` was not overriden, `CARGO_INSTALL_OPTS` is empty,
the package is in the default registry, and has no non-default configuration,
it will be used to install the package instead.

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

    List of packages to update in the [(registry_url):]package_name[:version] format.

    Registry defaults to the default crates.io registry,
    and can be a name from ~/.cargo/config.

    If specified in addition to --all,
    will add the specified packages to the update list
    (useful, e.g., in conjunction with --allow-no-update).

    Required if --all not given.

  -l --list

    Don't update any packages, just list them.

    If PACKAGE is empty, act as if --all was specified.

  -f --force

    Update all packages, regardless of whether they need to be version-wise.

  -d --downdate

    Downdate packages to match the latest unyanked version from the registry.

  -i --allow-no-update

    Allow to fresh install packages passed as PACKAGE argument.

    This is useful, for example, in pairing with cargo-install-update-config(1).

    Off by default.

  -g --git

    Also update git-originating packages.

    Off by default, because it's expensive.

  -q --quiet

    Don't print status messages to stdout
    and pass down --quiet to cargo subprocesses.

  --locked

    Enforce packages' embedded Cargo.lock files.
    This is equivalent to CARGO_INSTALL_OPTS=--locked (globally)
    and cargo-install-update-config(1) --enforce-lock (per package)
    except it doesn't disable cargo-binstall.

  -j --jobs <JOBS>...

    Run at most JOBS jobs at once, forwarded verbatim to cargo install.

  -s --filter <PACKAGE_FILTER>...

    Only consider packages matching all filters.

    PACKAGE_FILTER is in the form "key=value", where key is any of:
      - "toolchain": the package must be configured to be compiled with
                     the specified toolchain via cargo-install-update-config(1).

  -r --install-cargo <CARGO_EXECUTABLE>

    Cargo executable to run for installations.

    *Must* behave indistinguishably from the default cargo
    with regards to on-disk state ("$CARGO_DIR/.crates.toml"
    and installed executables) and the arguments it accepts.

    Required. Default: "cargo"

  -c --cargo-dir <CARGO_DIR>
     --root      <CARGO_DIR>

    Set the directory containing cargo metadata.

    Equivalent to, and forwarded as, cargo install --root.

    Required. Default: "$CARGO_INSTALL_ROOT", then "$CARGO_HOME",
    then "$HOME/.cargo", otherwise manual.

  -t --temp-dir <TEMP_DIR>

    Set the directory in which to clone git repositories.

    Adjoined with "cargo-update.$(id -un)" as last segment.

    Required. Default: system temp, otherwise manual.

## ENVIRONMENT VARIABLES

  `$CARGO_REGISTRIES_CRATES_IO_PROTOCOL`

    Overrides the registries.crates-io.protocol Cargo configuration key.

    The default is "sparse", and the crates.io URL is
    sparse+https://index.crates.io/.
    Set to some other value to use the git registry
    (https://github.com/rust-lang/crates.io-index) for crates.io.

  `$CARGO_NET_GIT_FETCH_WITH_CLI`

    Overrides the net.git-fetch-with-cli Cargo configuration key.

  `$GIT`

    Overrides the git executable in net.git-fetch-with-cli=true mode.

  `$CARGO_HTTP_CAINFO`

    Overrides the http.cainfo Cargo configuration key.

  `$CARGO_HTTP_CHECK_REVOKE`

    Overrides the http.check-revoke Cargo configuration key.

## EXAMPLES

  `cargo install-update -a`

    Update all installed packages.

    Example output:
          Polling registry 'https://index.crates.io/'........

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

  `cargo install-update racer treesize cargo-cln`

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
    ~/.cargo/bin/identicon.exe was removed manually, so a note is issued.

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

      identicon contains removed executables (identicon.exe), which will be re-installed on update ‒ you can remove it with cargo uninstall identicon

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
         Installing D:\Users\nabijaczleweli\.cargo\bin\checksums.exe

      Updating treesize
          Updating registry `https://github.com/rust-lang/crates.io-index`
         Downloading treesize v0.2.1
         [...]
         Compiling treesize v0.2.1
          Finished release [optimized] target(s) in 76.77 secs
         Replacing D:\Users\nabijaczleweli\.cargo\bin\treesize.exe

      Updated 2 packages.

  `cargo install-update -i (file:///usr/local/share/cargo):zram-generator:0.1.1`

    Install zram-generator from a local repository in /usr/local/share/cargo
    (but a remote one or a short name  will work just as well), at most version 0.1.1.

     Example output:
          Updating registry `file:///usr/local/share/cargo`

      Package         Installed  Latest   Needs update
      zram-generator             v0.1.1   Yes

      Installing zram-generator
          Updating registry `https://github.com/rust-lang/crates.io-index`
         Downloading zram-generator v0.1.1
         [...]
         Compiling zram-generator v0.1.1
          Finished release [optimized] target(s) in 21.62 secs
        Installing /home/nabijaczleweli/.cargo/bin/zram-generator
         Installed package `zram-generator v0.1.1` (executable `zram-generator`)

      Updated 1 package.

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

Written by наб &lt;<nabijaczleweli@nabijaczleweli.xyz>&gt;,
           Yann Simon &lt;<yann.simon.fr@gmail.com>&gt;,
           ven &lt;<vendethiel@hotmail.fr>&gt;,
           Cat Plus Plus &lt;<piotrlegnica@piotrl.pl>&gt;,
           Liigo &lt;<liigo@qq.com>&gt;,
           azyobuzin &lt;<azyobuzin@users.sourceforge.jp>&gt;,
           Tatsuyuki Ishi &lt;<ishitatsuyuki@gmail.com>&gt;,
           Tom Prince &lt;<tom.prince@twistedmatrix.com>&gt;,
           Mateusz Mikuła &lt;<mati865@gmail.com>&gt;,
           sinkuu &lt;<sinkuupump@gmail.com>&gt;,
           Alex Burka &lt;<aburka@seas.upenn.edu>&gt;,
           Matthias Krüger &lt;<matthias.krueger@famsik.de>&gt;,
           Daniel Holbert &lt;<dholbert@cs.stanford.edu>&gt;,
           Jonas Bushart &lt;<jonas@bushart.org>&gt;,
           Harrison Metzger &lt;<harrisonmetz@gmail.com>&gt;,
           Benjamin Bannier &lt;<bbannier@gmail.com>&gt;,
           Dimitris Apostolou &lt;<dimitris.apostolou@icloud.com>&gt;,
           Corbin Uselton &lt;<corbinu@decimal.io>&gt;,
           QuarticCat &lt;<QuarticCat@protonmail.com>&gt;,
           Artur Sinila &lt;<freesoftware@logarithmus.dev>&gt;,
           qthree &lt;<qthree3@gmail.com>&gt;,
           tranzystorekk &lt;<tranzystorek.io@protonmail.com>&gt;,
           Paul Barker &lt;<paul@pbarker.dev>&gt;,
           Benoît CORTIER &lt;<bcortier@proton.me>&gt;,
           Biswapriyo Nath &lt;<nathbappai@gmail.com>&gt;,
           Shiraz &lt;<smcclennon@protonmail.com>&gt;,
           Victor Song &lt;<vms2@rice.edu>&gt;,
           chrisalcantara &lt;<chris@chrisalcantara.com>&gt;,
           Utkarsh Gupta &lt;<utkarshgupta137@gmail.com>&gt;,
           nevsal,
           Rui Chen &lt;<https://chenrui.dev>&gt;,
           Lynnesbian &lt;<https://fedi.lynnesbian.space/@lynnesbian>&gt;,
           Rene Leonhardt,
       and Maxime Guerreiro &lt;<maxime@cloudflare.com>&gt;

## SPECIAL THANKS

To all who support further development, in particular:

  * ThePhD
  * Embark Studios
  * Lars Strojny
  * EvModder

## REPORTING BUGS

&lt;<https://github.com/nabijaczleweli/cargo-update/issues>&gt;

## SEE ALSO

&lt;<https://github.com/nabijaczleweli/cargo-update>&gt;

&lt;<https://github.com/ryankurte/cargo-binstall>&gt;
