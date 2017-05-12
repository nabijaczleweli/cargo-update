extern crate cargo_update;
extern crate tabwriter;
extern crate lazysort;
extern crate regex;
extern crate git2;

use std::process::{Command, exit};
use std::io::{Write, stdout};
use std::fs::{self, File};
use tabwriter::TabWriter;
use lazysort::SortedBy;
use git2::Repository;
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

    let crates_file = cargo_update::ops::resolve_crates_file(opts.crates_file.1);
    let configuration = try!(cargo_update::ops::PackageConfig::read(&crates_file.with_file_name(".install_config.toml")));
    let mut packages = cargo_update::ops::installed_main_repo_packages(&crates_file);

    if !opts.to_update.is_empty() {
        packages = cargo_update::ops::intersect_packages(packages, &opts.to_update, opts.install);
    }

    {
        // Searching for "" will just update the registry
        let search_res = Command::new("cargo").arg("search").arg("").status().unwrap();
        if !search_res.success() {
            return Err(search_res.code().unwrap_or(-1));
        }
        println!("");
    }

    let registry = cargo_update::ops::get_index_path(&opts.cargo_dir.1);
    let registry_repo = try!(Repository::open(&registry).map_err(|_| {
        println!("Failed to open registry repository at {}.", registry.display());
        2
    }));
    let latest_registry = try!(registry_repo.revparse_single("origin/master").map_err(|_| {
        println!("Failed read master branch of registry repositry at {}.", registry.display());
        2
    }));

    for package in &mut packages {
        package.pull_version(&latest_registry.as_commit().unwrap().tree().unwrap(), &registry_repo);
    }

    {
        let mut out = TabWriter::new(stdout());
        writeln!(out, "Package\tInstalled\tLatest\tNeeds update").unwrap();
        for package in packages.iter().sorted_by(|lhs, rhs| (!lhs.needs_update()).cmp(&!rhs.needs_update())) {
            write!(out, "{}\t", package.name).unwrap();
            if let Some(ref v) = package.version {
                write!(out, "v{}", v).unwrap();
            }
            writeln!(out,
                     "\tv{}\t{}",
                     package.newest_version.as_ref().unwrap(),
                     if package.needs_update() { "Yes" } else { "No" })
                .unwrap();
        }
        writeln!(out, "").unwrap();
        out.flush().unwrap();
    }

    if opts.update {
        if !opts.force {
            packages = packages.into_iter().filter(cargo_update::ops::MainRepoPackage::needs_update).collect();
        }

        if !packages.is_empty() {
            let (success_n, errored, result): (usize, Vec<String>, Option<i32>) = packages.into_iter()
                .map(|package| -> Result<(), (i32, String)> {
                    println!("{} {}",
                             if package.version.is_some() {
                                 "Updating"
                             } else {
                                 "Installing"
                             },
                             package.name);

                    if cfg!(target_os = "windows") && package.version.is_some() && package.name == "cargo-update" {
                        let cur_exe = env::current_exe().unwrap();
                        fs::rename(&cur_exe, cur_exe.with_extension(format!("exe-v{}", package.version.as_ref().unwrap()))).unwrap();
                        // This way the past-current exec will be "replaced" we'll get no dupes in .cargo.toml
                        File::create(cur_exe).unwrap();
                    }

                    let install_res = if let Some(cfg) = configuration.get(&package.name) {
                        Command::new("cargo").args(cfg.cargo_args()).arg(&package.name).status().unwrap()
                    } else {
                        Command::new("cargo").arg("install").arg("-f").arg(&package.name).status().unwrap()
                    };

                    println!("");
                    if !install_res.success() {
                        if cfg!(target_os = "windows") && package.version.is_some() && package.name == "cargo-update" {
                            let cur_exe = env::current_exe().unwrap();
                            fs::remove_file(&cur_exe).unwrap();
                            fs::rename(cur_exe.with_extension(format!("exe-v{}", package.version.as_ref().unwrap())), cur_exe).unwrap();
                        }

                        Err((install_res.code().unwrap_or(-1), package.name))
                    } else {
                        Ok(())
                    }
                })
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
                return Err(result.unwrap());
            }
        } else {
            println!("No packages need updating.");
        }
    }

    Ok(())
}
