extern crate cargo_update;

use std::process::exit;


fn main() {
    let result = actual_main().err().unwrap_or(0);
    exit(result);
}

fn actual_main() -> Result<(), i32> {
    let opts = cargo_update::Options::parse();
    println!("{:?}", opts);

    let mut packages = cargo_update::ops::installed_main_repo_packages(&opts.cargo_dir.1);

    if !opts.to_update.is_empty() {
        packages = cargo_update::ops::intersect_packages(packages, &opts.to_update);
    }

    for package in &packages {
        println!("{} v{}", package.name, package.version);
    }

    let token = try!(cargo_update::ops::crates_token(&opts.cargo_dir.1));

    Ok(())
}
