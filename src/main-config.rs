extern crate cargo_update;
extern crate tabwriter;

use std::io::{Write, stdout};
use tabwriter::TabWriter;
use std::process::exit;


fn main() {
    let result = actual_main().err().unwrap_or(0);
    exit(result);
}

fn actual_main() -> Result<(), i32> {
    let opts = cargo_update::ConfigOptions::parse();
    let config_file = cargo_update::ops::crates_file_in(&opts.cargo_dir).with_file_name(".install_config.toml");

    let mut configuration = cargo_update::ops::PackageConfig::read(&config_file).map_err(|(e, r)| {
            eprintln!("Reading config: {}", e);
            r
        })?;
    if !opts.ops.is_empty() {
        if *configuration.entry(opts.package.clone())
            .and_modify(|cfg| cfg.execute_operations(&opts.ops))
            .or_insert_with(|| cargo_update::ops::PackageConfig::from(&opts.ops)) == Default::default() {
            configuration.remove(&opts.package);
        }

        cargo_update::ops::PackageConfig::write(&configuration, &config_file).map_err(|(e, r)| {
                eprintln!("Writing config: {}", e);
                r
            })?;
    }

    if let Some(cfg) = configuration.get(&opts.package) {
        let mut out = TabWriter::new(stdout());
        if let Some(ref t) = cfg.toolchain {
            writeln!(out, "Toolchain\t{}", t).unwrap();
        }
        if let Some(d) = cfg.debug {
            writeln!(out, "Debug mode\t{}", d).unwrap();
        }
        if let Some(ip) = cfg.install_prereleases {
            writeln!(out, "Install prereleases\t{}", ip).unwrap();
        }
        if let Some(el) = cfg.enforce_lock {
            writeln!(out, "Enforce lock\t{}", el).unwrap();
        }
        if let Some(rb) = cfg.respect_binaries {
            writeln!(out, "Respect binaries\t{}", rb).unwrap();
        }
        if let Some(ref tv) = cfg.target_version {
            writeln!(out, "Target version\t{}", tv).unwrap();
        }
        writeln!(out, "Default features\t{}", cfg.default_features).unwrap();
        if !cfg.features.is_empty() {
            write!(out, "Features").unwrap();
            for f in &cfg.features {
                writeln!(out, "\t{}", f).unwrap();
            }
        }
        if let Some(env) = cfg.environment.as_ref() {
            if !env.is_empty() {
                write!(out, "Environment variables").unwrap();
                for (var, val) in env {
                    match val {
                        cargo_update::ops::EnvironmentOverride(Some(val)) => writeln!(out, "\t{}={}", var, val).unwrap(),
                        cargo_update::ops::EnvironmentOverride(None) => writeln!(out, "\t{}\tcleared", var).unwrap(),
                    }
                }
            }
        }
        out.flush().unwrap();
    } else {
        println!("No configuration for package {}.", opts.package);
    }

    Ok(())
}
