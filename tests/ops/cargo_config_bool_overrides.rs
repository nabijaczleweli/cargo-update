// CARGO_NET_GIT_FETCH_WITH_CLI and CARGO_HTTP_CHECK_REVOKE must be read exactly as cargo reads them.
//
// These go through CargoConfig::load_with_bool_env, which is the whole of CargoConfig::load (reading config.toml, truthy(),
// defaults) except that the two variables come from the test instead of the process environment.

use cargo_update::ops::CargoConfig;
use std::env::temp_dir;
use std::path::{Path, PathBuf};
use std::ffi::OsString;
use std::fs;


const FETCH_WITH_CLI: &str = "CARGO_NET_GIT_FETCH_WITH_CLI";
const CHECK_REVOKE: &str = "CARGO_HTTP_CHECK_REVOKE";

/// The boolean override variables set for one `resolve()`: `vars![]` for none, or e.g.
/// `vars![FETCH_WITH_CLI => "true", CHECK_REVOKE => "false"]`.
macro_rules! vars {
    ($($var:expr => $value:expr),* $(,)?) => {
        vec![$(($var, OsString::from($value))),*]
    };
}

fn invalid_value_error(var: &str) -> String {
    format!("error in environment variable `{}`: provided string was not `true` or `false`", var)
}

/// The two resolved values, named so that assertions don't depend on argument order.
#[derive(Debug, PartialEq)]
struct Resolved {
    fetch_with_cli: bool,
    check_revoke: bool,
}

/// http.check-revoke's default, when neither the variable nor the config file sets it.
const DEFAULT_CHECK_REVOKE: bool = cfg!(target_os = "windows");

const CONFIG_ALL_TRUE: &str = "net.git-fetch-with-cli = true\nhttp.check-revoke = true\n";
const CONFIG_ALL_FALSE: &str = "net.git-fetch-with-cli = false\nhttp.check-revoke = false\n";

/// The scratch directory for one test, where it writes the config.toml it resolves against.
fn test_dir(test: &str) -> PathBuf {
    let td = temp_dir().join("cargo_update-test").join(format!("cargo_config_bool_overrides-{}", test));
    let _ = fs::create_dir_all(&td);
    td
}

/// Resolve the configuration with `config` written to `dir`'s config.toml and `env` as the only boolean override
/// variables that are set.
fn resolve(dir: &Path, config: &str, env: Vec<(&str, OsString)>) -> Result<Resolved, String> {
    fs::write(dir.join("config.toml"), config).unwrap();

    // load() takes the path of cargo's .crates.toml and reads the config next to it; the file itself need not exist.
    CargoConfig::load_with_bool_env(&dir.join(".crates.toml"),
                                    |name| env.iter().find(|(k, _)| *k == name).map(|(_, v)| v.clone()))
        .map(|c| {
            Resolved {
                fetch_with_cli: c.net_git_fetch_with_cli,
                check_revoke: c.http.check_revoke,
            }
        })
}


#[test]
fn unset_uses_config_or_default() {
    let dir = test_dir("unset_uses_config_or_default");
    assert_eq!(resolve(&dir, "", vars![]),
               Ok(Resolved { fetch_with_cli: false, check_revoke: DEFAULT_CHECK_REVOKE }));
    assert_eq!(resolve(&dir, CONFIG_ALL_TRUE, vars![]),
               Ok(Resolved { fetch_with_cli: true, check_revoke: true }));
    assert_eq!(resolve(&dir, CONFIG_ALL_FALSE, vars![]),
               Ok(Resolved { fetch_with_cli: false, check_revoke: false }));
}

#[test]
fn booleans_override_config() {
    let dir = test_dir("booleans_override_config");
    assert_eq!(resolve(&dir, CONFIG_ALL_FALSE, vars![FETCH_WITH_CLI => "true", CHECK_REVOKE => "true"]),
               Ok(Resolved { fetch_with_cli: true, check_revoke: true }));
    assert_eq!(resolve(&dir, CONFIG_ALL_TRUE, vars![FETCH_WITH_CLI => "false", CHECK_REVOKE => "false"]),
               Ok(Resolved { fetch_with_cli: false, check_revoke: false }));
}

#[test]
fn each_variable_sets_its_own_key() {
    let dir = test_dir("each_variable_sets_its_own_key");
    assert_eq!(resolve(&dir, "", vars![FETCH_WITH_CLI => "true"]),
               Ok(Resolved { fetch_with_cli: true, check_revoke: DEFAULT_CHECK_REVOKE }));
    assert_eq!(resolve(&dir, "", vars![CHECK_REVOKE => "true"]),
               Ok(Resolved { fetch_with_cli: false, check_revoke: true }));
    assert_eq!(resolve(&dir, "", vars![CHECK_REVOKE => "false"]),
               Ok(Resolved { fetch_with_cli: false, check_revoke: false }));
}

#[test]
fn anything_else_is_an_error() {
    let dir = test_dir("anything_else_is_an_error");
    for var in [FETCH_WITH_CLI, CHECK_REVOKE] {
        for value in ["", "1", "0", "yes", "no", "TRUE", "False", " true", "false ", "false\n", "\"true\"", "a=true"] {
            for config in ["", CONFIG_ALL_TRUE, CONFIG_ALL_FALSE] {
                assert_eq!(resolve(&dir, config, vars![var => value]),
                           Err(invalid_value_error(var)),
                           "{}={:?} config={:?}",
                           var,
                           value,
                           config);
            }
        }
    }
}

/// The config file value is interpreted loosely by `truthy()`. This knowingly differs from cargo, which rejects anything
/// but a TOML boolean there.
#[test]
fn config_value_is_read_loosely() {
    let dir = test_dir("config_value_is_read_loosely");
    let config_string_false = "net.git-fetch-with-cli = \"false\"\nhttp.check-revoke = \"false\"\n";
    assert_eq!(resolve(&dir, config_string_false, vars![]),
               Ok(Resolved { fetch_with_cli: true, check_revoke: true }));
    assert_eq!(resolve(&dir, "net.git-fetch-with-cli = 0\nhttp.check-revoke = 0\n", vars![]),
               Ok(Resolved { fetch_with_cli: false, check_revoke: false }));
    assert_eq!(resolve(&dir, config_string_false, vars![FETCH_WITH_CLI => "false", CHECK_REVOKE => "false"]),
               Ok(Resolved { fetch_with_cli: false, check_revoke: false }));
}

#[cfg(unix)]
#[test]
fn not_utf8_needs_config() {
    use std::os::unix::ffi::OsStringExt;

    let dir = test_dir("not_utf8_needs_config");
    let not_utf8 = OsString::from_vec(b"tr\xffue".to_vec());
    for (var, key) in [(FETCH_WITH_CLI, "net.git-fetch-with-cli"), (CHECK_REVOKE, "http.check-revoke")] {
        assert_eq!(resolve(&dir, CONFIG_ALL_FALSE, vars![var => not_utf8.clone()]),
                   Ok(Resolved { fetch_with_cli: false, check_revoke: false }),
                   "{}",
                   var);
        assert_eq!(resolve(&dir, CONFIG_ALL_TRUE, vars![var => not_utf8.clone()]),
                   Ok(Resolved { fetch_with_cli: true, check_revoke: true }),
                   "{}",
                   var);
        assert_eq!(resolve(&dir, "", vars![var => not_utf8.clone()]), Err(format!("missing config key `{}`", key)));
    }
}
