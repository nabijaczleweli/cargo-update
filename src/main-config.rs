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
    let config_file = cargo_update::ops::resolve_crates_file(opts.crates_file.1).with_file_name(".install_config.toml");

    let mut configuration = cargo_update::ops::PackageConfig::read(&config_file).map_err(|(e, r)| {
            eprintln!("Reading config: {}", e);
            r
        })?;
    if !opts.ops.is_empty() {
        let mut changed = false;
        if let Some(ref mut cfg) = configuration.get_mut(&opts.package) {
            cfg.execute_operations(&opts.ops);
            changed = true;
        }
        if !changed {
            configuration.insert(opts.package.clone(), cargo_update::ops::PackageConfig::from(&opts.ops));
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
        out.flush().unwrap();
    } else {
        println!("No configuration for package {}.", opts.package);
    }

    Ok(())
}
