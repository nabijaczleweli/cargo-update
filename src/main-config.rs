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

    let mut configuration = try!(cargo_update::ops::PackageConfig::read(&config_file));
    if !opts.ops.is_empty() {
        let mut changed = false;
        if let Some(ref mut cfg) = configuration.get_mut(&opts.package) {
            cfg.execute_operations(&opts.ops);
            changed = true;
        }
        if !changed {
            configuration.insert(opts.package.clone(), cargo_update::ops::PackageConfig::from(&opts.ops));
        }

        try!(cargo_update::ops::PackageConfig::write(&configuration, &config_file));
    }

    if let Some(cfg) = configuration.get(&opts.package) {
        let mut out = TabWriter::new(stdout());
        if let Some(ref t) = cfg.toolchain {
            writeln!(out, "Toolchain\t{}", t).unwrap();
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
