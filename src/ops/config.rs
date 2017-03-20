use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::fs::File;
use toml;


#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigOperation {
    SetToolchain(String),
    RemoveToolchain,
    DefaultFeatures(bool),
    AddFeature(String),
    RemoveFeature(String),
}


/// A leaderboard entry.
///
/// # Examples
///
/// Reading a leaderboard, adding an entry to it, then writing it back.
///
/// ```
/// # use std::fs::{File, create_dir_all};
/// # use poke_a_mango::ops::Leader;
/// # use std::env::temp_dir;
/// let tf = temp_dir().join("poke-a-mango-doctest").join("ops-Leader-0");
/// create_dir_all(&tf).unwrap();
///
/// let tf = tf.join("leaderboard.toml");
/// File::create(&tf).unwrap();
///
/// let mut leaders = Leader::read(&tf).unwrap();
/// assert!(leaders.is_empty());
/// leaders.push(Leader::now("nabijaczleweli".to_string(), 105));
/// assert_eq!(Leader::write(leaders, &tf), Ok(()));
/// ```
///
/// This could alternatively be done with `Leader::append()`.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PackageConfig {
    pub toolchain: Option<String>,
    pub default_features: bool,
    pub features: Vec<String>,
}


impl PackageConfig {
    /// Read leaderboard from the specified file.
    ///
    /// If the specified file doesn't exist an empty leaderboard is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::fs::{File, create_dir_all};
    /// # use poke_a_mango::ops::Leader;
    /// # use std::env::temp_dir;
    /// let tf = temp_dir().join("poke-a-mango-doctest").join("ops-Leader-read-0");
    /// create_dir_all(&tf).unwrap();
    ///
    /// let tf = tf.join("leaderboard.toml");
    /// File::create(&tf).unwrap();
    ///
    /// assert_eq!(Leader::read(&tf), Ok(vec![]));
    /// ```
    pub fn read(p: &Path) -> Result<BTreeMap<String, PackageConfig>, i32> {
        if p.exists() {
            let mut buf = String::new();
            try!(try!(File::open(p).map_err(|_| 1))
                .read_to_string(&mut buf)
                .map_err(|_| 1));

            toml::from_str(&buf).map_err(|_| 2)
        } else {
            Ok(BTreeMap::new())
        }
    }

    /// Save leaderboard to the specified file.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate poke_a_mango;
    /// # extern crate chrono;
    /// # use std::fs::{File, create_dir_all};
    /// # use self::chrono::{Duration, Local};
    /// # use self::poke_a_mango::ops::Leader;
    /// # use std::env::temp_dir;
    /// # fn main() {
    /// let tf = temp_dir().join("poke-a-mango-doctest").join("ops-Leader-write-0");
    /// create_dir_all(&tf).unwrap();
    ///
    /// Leader::write(vec![Leader::now("nabijaczleweli".to_string(), 105),
    ///                    Leader::now("skorezore".to_string(), 51)],
    ///               &tf.join("leaderboard.toml"))
    ///     .unwrap();
    /// # }
    /// ```
    pub fn write(queued_leaders: &BTreeMap<String, PackageConfig>, p: &Path) -> Result<(), i32> {
        try!(File::create(p).map_err(|_| 3))
            .write_all(&try!(toml::to_vec(queued_leaders).map_err(|_| 2)))
            .map_err(|_| 3)
    }
}
