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
    let opts = cargo_update::Options::parse();

    let mut packages = cargo_update::ops::installed_main_repo_packages(&opts.cargo_dir.1);

    if !opts.to_update.is_empty() {
        packages = cargo_update::ops::intersect_packages(packages, &opts.to_update);
    }

    let token = try!(cargo_update::ops::crates_token(&opts.cargo_dir.1));

    for package in &mut packages {
        package.pull_version(&token);
    }
    {
        let mut out = TabWriter::new(stdout());
        writeln!(out, "Package\tInstalled\tLatest").unwrap();
        for package in &packages {
            writeln!(out, "{}\tv{}\tv{}", package.name, package.version, package.newest_version.as_ref().unwrap()).unwrap();
        }
        out.flush().unwrap();
    }

    Ok(())
}
