cargo-install-update-config(1) -- Cargo subcommand for checking and applying updates to installed executables -- configuration
==============================================================================================================================

## SYNOPSIS

`cargo install-update-config` [OPTIONS] <PACKAGE>

## DESCRIPTION

Configure cargo/rustc compilation commandlines for packages.

Settable options:

  * toolchain,
  * whether to use default features,
  * additional feature list.

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

  --release

    Compile in release mode (default).

  -v --version [VERSION_REQ]

    Require a cargo-compatible version range not to update beyond.

    Example: ">1.3", "^0.1.8".

  -a --any-version

    Allow any version.

  -c --cargo-dir <CARGO_DIR>

    Set the directory containing cargo metadata.

    Required. Default: "$CARGO_HOME", then "$HOME/.cargo", otherwise manual.

## EXAMPLES

  `cargo install-update-config -t nightly -d 0 -f log -f colour -v ~2.3 clippy`

    Set clippy to be compiled with the nightly toolchain without default
    features, with log and colour features.

    Example output:
      Toolchain         nightly
      Default features  true
      Features          log
                        colour

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

## REPORTING BUGS

&lt;<https://github.com/nabijaczleweli/cargo-update/issues>&gt;

## SEE ALSO

&lt;<https://github.com/nabijaczleweli/cargo-update>&gt;
