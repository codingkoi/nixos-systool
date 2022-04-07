use chrono::prelude::*;
use chrono::Duration;
use color_eyre::eyre::eyre;
use color_eyre::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
pub struct FlakeLock {
    nodes: HashMap<String, InputNode>,
}

#[derive(Deserialize)]
struct InputNode {
    /// Lock information for this input
    ///
    /// This is an Option because "root" is a special case
    /// node in the lock file.
    locked: Option<InputLock>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputLock {
    /// Timestamp of when this input was last updated
    last_modified: i64,
}

pub enum FlakeStatus {
    UpToDate,
    Outdated { since: Date<Utc> },
}

impl FlakeLock {
    /// Load the flake.lock file into a representation we can use
    pub fn load<T: AsRef<Path>>(filename: T) -> Result<Self> {
        let content = fs::read_to_string(filename)?;
        Ok(serde_json::from_str::<Self>(&content)?)
    }

    pub fn check(&self) -> Result<FlakeStatus> {
        if let Some(nixpkgs) = self.nodes.get("nixpkgs") {
            let now = Utc::now();
            // Have to jump through hoops because DateTime doesn't
            // implement From for integers, probably for good reason.
            let last_update_ts = NaiveDateTime::from_timestamp(
                nixpkgs
                    .locked
                    .as_ref()
                    .expect("`nixpkgs` is missing a `locked` section in flake lock!")
                    .last_modified,
                0,
            );
            let last_update = DateTime::from_utc(last_update_ts, Utc);
            if now - last_update >= Duration::weeks(2) {
                Ok(FlakeStatus::Outdated {
                    since: last_update.date(),
                })
            } else {
                Ok(FlakeStatus::UpToDate)
            }
        } else {
            Err(eyre!("Cannot find 'nixpkgs' in flake lock!"))
        }
    }
}
