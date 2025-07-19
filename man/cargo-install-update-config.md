cargo-install-update-config(1) -- Cargo subcommand for checking and applying updates to installed executables -- configuration
==============================================================================================================================

## SYNOPSIS

`cargo install-update-config` [OPTIONS] <PACKAGE>

## DESCRIPTION

Configure cargo/rustc compilation commandlines for packages.

Settable options:

  * toolchain,
  * whether to use default features,
  * additional feature list,
  * build profile,
  * whether to install prereleases other than those for the currently-installed version,
  * Cargo.lock enforcement,
  * version range locks,
  * environment variable value or removal.

If there is no configuration for a package,
the `$CARGO_DIR/.crates2.json` file is parsed instead,
which may yield, depending on the Cargo version, the following subset of the data:

  * whether to use default features,
  * additional feature list,
  * build profile.

See cargo-install-update(1) for general information.

## OPTIONS

  <PACKAGE>

    Package to adjust settings for.

  -t --toolchain [TOOLCHAIN]

    Set the toolchain to use. Pass empty string to use the cargo default.

  -f --feature [FEATURE]...

    Enable a cargo feature.

  -n --no-feature [FEATURE]...

    Disable/remove a cargo feature.

  -d --default-features [DEFAULT]

    Enable or disable default features.

    The argument can have the value "yes", "true", "1" to enable,
    or "no", "false", "0" to disable.

  --debug

    Compile in debug mode.
    Same as --build-profile dev.

  --release

    Compile in release mode (default).
    Same as --build-profile release.

  --build-profile [PROFILE]

    Compile with PROFILE
    (dev/release/test/bench or defined in $CARGO_DIR/.cargo/config.toml under [profile.PROFILE]).

  --install-prereleases

    Install version even if it's a prerelease.

  --no-install-prereleases

    Don't update to prerelease versions.

    If the currently-installed version is a prerelease,
    and the candidate version is a newer prerelease for the same major.minor.patch version,
    it will be installed regardless of this setting.
    (To wit: this setting controls updates to prereleases, not within them.)

  --enforce-lock

    Require Cargo.lock is up to date.

  --no-enforce-lock

    Don't require Cargo.lock to be up to date. (default).

  --respect-binaries

    Only install the binaries that are already installed for this package.

  --no-respect-binaries

    Install all binaries. (default).

  -v --version [VERSION_REQ]

    Require a cargo-compatible version range not to update beyond.

    Example: ">1.3", "^0.1.8".

  -a --any-version

    Allow any version.

  -e --environment [VARIABLE=VALUE]...

    Set environment VARIABLE to VALUE in the cargo install process.

  -E --clear-environment [VARIABLE]...

    Remove environment VARIABLE from the cargo install process.

  --inherit-environment [VARIABLE]...

    Don't do anything to environment VARIABLE.

  -r --reset

    Roll back the configuration to the empty defaults.

  -c --cargo-dir <CARGO_DIR>

    Set the directory containing cargo metadata.

    Required. Default: "$CARGO_HOME", then "$HOME/.cargo", otherwise manual.

## EXAMPLES

  `cargo install-update-config -t nightly -d 0 -f log -f colour -v ~2.3 -e RUSTC_WRAPPER=sccache clippy`

    Set clippy to be compiled with the nightly toolchain without default
    features, with log and colour features.

    Example output:
      Toolchain              nightly
      Default features       true
      Features               log
                             colour
      Environment variables  RUSTC_WRAPPER=sccache

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
       and Rene Leonhardt

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
