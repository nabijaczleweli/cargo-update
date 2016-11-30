// This is so cargo actually considers linking to the fucking manifest,
// since the build scripts' documentation can do the digital equivalent of fucking off
#[cfg(target_os="windows")]
#[link(name="cargo-install-update-manifest", kind="static")]
extern "C" {}


extern crate cargo_update;
extern crate tabwriter;
extern crate regex;

use std::process::{Command, exit};
use std::io::{Write, stdout};
use std::fs::{self, File};
use tabwriter::TabWriter;
use regex::Regex;
use std::env;


fn main() {
    let result = actual_main().err().unwrap_or(0);
    exit(result);
}

fn actual_main() -> Result<(), i32> {
    let opts = cargo_update::Options::parse();

    if cfg!(target_os = "windows") {
        let old_version_r = Regex::new(r"cargo-install-update\.exe-v.+").unwrap();
        for old_version in fs::read_dir(env::current_exe().unwrap().parent().unwrap().canonicalize().unwrap())
            .unwrap()
            .map(Result::unwrap)
            .filter(|f| old_version_r.is_match(&f.file_name().into_string().unwrap())) {
            fs::remove_file(old_version.path()).unwrap();
        }
    }

    let mut packages = cargo_update::ops::installed_main_repo_packages(&opts.cargo_dir.1);

    if !opts.to_update.is_empty() {
        packages = cargo_update::ops::intersect_packages(packages, &opts.to_update);
    }

    {
        // Searching for "" will just update the registry
        let search_res = Command::new("cargo").arg("search").arg("").status().unwrap();
        if !search_res.success() {
            try!(Err(search_res.code().unwrap_or(-1)));
        }
        println!("");
    }

    let registry = cargo_update::ops::get_index_path(&opts.cargo_dir.1);

    for package in &mut packages {
        package.pull_version(&registry);
    }

    {
        let mut out = TabWriter::new(stdout());
        writeln!(out, "Package\tInstalled\tLatest\tNeeds update").unwrap();
        for package in &packages {
            writeln!(out,
                     "{}\tv{}\tv{}\t{}",
                     package.name,
                     package.version,
                     package.newest_version.as_ref().unwrap(),
                     if package.version < *package.newest_version.as_ref().unwrap() {
                         "Yes"
                     } else {
                         "No"
                     })
                .unwrap();
        }
        writeln!(out, "").unwrap();
        out.flush().unwrap();
    }

    if opts.update {
        if !opts.force {
            packages = packages.into_iter().filter(|pkg| pkg.version < *pkg.newest_version.as_ref().unwrap()).collect();
        }

        if !packages.is_empty() {
            let (success_n, errored, result): (usize, Vec<String>, Option<i32>) = packages.into_iter()
                .map(|package| -> Result<(), (i32, String)> {
                    println!("Updating {}", package.name);

                    if cfg!(target_os = "windows") && package.name == "cargo-update" {
                        let cur_exe = env::current_exe().unwrap();
                        let mut new_exe = cur_exe.clone();

                        new_exe.set_extension(format!("exe-v{}", package.version));
                        fs::rename(&cur_exe, new_exe).unwrap();
                        // This way the past-current exec will be "replaced" we'll get no dupes in .cargo.toml
                        File::create(cur_exe).unwrap();
                    }

                    let install_res = Command::new("cargo").arg("install").arg("-f").arg(&package.name).status().unwrap();
                    if !install_res.success() {
                        try!(Err((install_res.code().unwrap_or(-1), package.name)));
                    }

                    println!("");

                    Ok(())
                })
                .collect::<Vec<_>>()
                .into_iter()
                .fold((0, vec![], None), |(s, mut e, r), p| match p {
                    Ok(()) => (s + 1, e, r),
                    Err((pr, pn)) => {
                        e.push(pn);
                        (s, e, r.or(Some(pr)))
                    }
                });

            println!("");
            println!("Updated {} package{}.", success_n, if success_n == 1 { "" } else { "s" });
            if !errored.is_empty() && result.is_some() {
                println!("Failed to update {}.", &errored.iter().fold("".to_string(), |s, e| s + ", " + e)[2..]);
                try!(Err(result.unwrap()));
            }
        } else {
            println!("No packages need updating.");
        }
    }

    Ok(())
}
