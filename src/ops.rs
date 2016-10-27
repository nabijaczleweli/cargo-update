use hyper::header::{Authorization, Bearer};
use hyper::Client as HttpClient;
use semver::Version as Semver;
use std::path::Path;
use std::fs::File;
use std::io::Read;
use regex::Regex;
use toml;
use json;


lazy_static! {
    static ref PACKAGE_RGX: Regex = Regex::new(r"([^\s]+) ([^\s]+) \(([^+\s]+)+\+([^\s]+)\)").unwrap();
}


#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct MainRepoPackage {
    pub name: String,
    pub version: Semver,
    pub newest_version: Option<Semver>,
}

impl MainRepoPackage {
    pub fn parse(what: &str) -> Option<MainRepoPackage> {
        PACKAGE_RGX.captures(what).and_then(|c| if c.at(3).unwrap() == "registry" {
            Some(MainRepoPackage {
                name: c.at(1).unwrap().to_string(),
                version: Semver::parse(c.at(2).unwrap()).unwrap(),
                newest_version: None,
            })
        } else {
            None
        })
    }

    pub fn pull_version(&mut self, crates_token: &str) {
        let vers = crate_versions(crate_versions_raw(crates_token, &self.name));
        self.newest_version = vers.into_iter().max();
    }
}


pub fn installed_main_repo_packages(cargo_dir: &Path) -> Vec<MainRepoPackage> {
    let crates_path = cargo_dir.join(".crates.toml");
    if crates_path.exists() {
        let mut crates = String::new();
        File::open(crates_path).unwrap().read_to_string(&mut crates).unwrap();

        toml::Parser::new(&crates).parse().unwrap()["v1"].as_table().unwrap().keys().flat_map(|s| MainRepoPackage::parse(s)).collect()
    } else {
        Vec::new()
    }
}

pub fn crates_token(cargo_dir: &Path) -> Result<String, i32> {
    let config_path = cargo_dir.join("config");
    if config_path.exists() {
        let mut config = String::new();
        File::open(config_path).unwrap().read_to_string(&mut config).unwrap();

        Ok(toml::Parser::new(&config).parse().unwrap()["registry"].as_table().unwrap()["token"].as_str().unwrap().to_string())
    } else {
        Err(1)
    }
}

pub fn intersect_packages(installed: Vec<MainRepoPackage>, to_update: &Vec<String>) -> Vec<MainRepoPackage> {
    installed.into_iter().filter(|p| to_update.contains(&p.name)).collect()
}

pub fn crate_versions_raw(token: &str, crate_name: &str) -> String {
    let mut buf = String::new();
    HttpClient::new()
        .get(&format!("https://crates.io/api/v1/crates/{}/versions", crate_name))
        .header(Authorization(Bearer { token: token.to_string() }))
        .send()
        .unwrap()
        .read_to_string(&mut buf)
        .unwrap();
    buf
}

pub fn crate_versions(raw: String) -> Vec<Semver> {
    json::parse(&raw).unwrap()["versions"].members().map(|v| Semver::parse(v["num"].as_str().unwrap()).unwrap()).collect()
}
