use chrono::prelude::*;
use chrono::Duration;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Deserialize)]
/// Flake lock file
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
    UpToDate {
        last_update: NaiveDate,
        since: Duration,
    },
    Outdated {
        last_update: NaiveDate,
        since: Duration,
    },
}

#[derive(Error, Debug)]
pub enum FlakeLoadError {
    #[error("Couldn't read lock file: {0}")]
    LockFileError(#[from] std::io::Error),
    #[error("Failed to parse lock file JSON: {0}")]
    JsonParseError(#[from] serde_json::Error),
}

#[derive(Error, Debug)]
pub enum FlakeCheckError {
    #[error("Cannot find 'nixpkgs' in flake lock!")]
    NixpkgsNotFound,
}

impl FlakeLock {
    /// Load the flake.lock file into a representation we can use
    pub fn load<T: AsRef<Path>>(filename: T) -> Result<Self, FlakeLoadError> {
        let content = fs::read_to_string(filename)?;
        Ok(serde_json::from_str::<Self>(&content)?)
    }

    pub fn check(&self, allowed_age: u32) -> Result<FlakeStatus, FlakeCheckError> {
        if let Some(nixpkgs) = self.nodes.get("nixpkgs") {
            let now = Utc::now();
            let last_update_ts = NaiveDateTime::from_timestamp_opt(
                nixpkgs
                    .locked
                    .as_ref()
                    .expect("`nixpkgs` input is missing a `locked` section in flake lock!")
                    .last_modified,
                0,
            );
            let last_update = DateTime::from_utc(
                last_update_ts
                    .expect("Couldn't find or parse last modified time for `nixpkgs` input."),
                Utc,
            );
            let duration = now - last_update;
            if duration >= Duration::days(allowed_age as i64) {
                Ok(FlakeStatus::Outdated {
                    last_update: last_update.date_naive(),
                    since: duration,
                })
            } else {
                Ok(FlakeStatus::UpToDate {
                    last_update: last_update.date_naive(),
                    since: duration,
                })
            }
        } else {
            Err(FlakeCheckError::NixpkgsNotFound)
        }
    }
}
