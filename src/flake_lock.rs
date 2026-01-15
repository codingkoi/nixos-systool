// SPDX-License-Identifier: GPL-3.0-or-later

use chrono::prelude::*;
use chrono::Duration;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Deserialize, Debug)]
/// Flake lock file
pub struct FlakeLock {
    nodes: HashMap<String, InputNode>,
}

#[derive(Deserialize, Debug)]
struct InputNode {
    /// Lock information for this input
    ///
    /// This is an Option because "root" is a special case
    /// node in the lock file.
    locked: Option<InputLock>,
    /// Input map from the "root" node.
    inputs: Option<HashMap<String, InputValue>>,
}

#[derive(Deserialize, Debug, Hash, PartialEq, Eq)]
#[serde(untagged)]
enum InputValue {
    Single(String),
    // I'm not sure what these are used for, but they don't occur
    // in the "root" node which is all I care about.
    List(Vec<String>),
}

#[derive(Deserialize, Debug)]
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

impl FlakeStatus {
    pub fn last_update(&self) -> &NaiveDate {
        match self {
            FlakeStatus::UpToDate { last_update, .. }
            | FlakeStatus::Outdated { last_update, .. } => last_update,
        }
    }
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
    #[error("Found unexpected list in 'root.inputs.nixpkgs'!")]
    FoundListInRoot,
}

impl FlakeLock {
    /// Load the flake.lock file into a representation we can use
    pub fn load<T: AsRef<Path>>(filename: T) -> Result<Self, FlakeLoadError> {
        let content = fs::read_to_string(filename)?;
        Ok(serde_json::from_str::<Self>(&content)?)
    }

    fn lookup_nixpkgs(&self) -> Result<&InputNode, FlakeCheckError> {
        if let Some(root_mapping) = self.nodes.get("root") {
            let nixpkgs_name = root_mapping
                .inputs
                .as_ref()
                .expect("`root` is missing a `inputs` section in flake lock!")
                .get("nixpkgs")
                .unwrap();
            match nixpkgs_name {
                InputValue::Single(nixpkgs_name) => Ok(self
                    .nodes
                    .get(nixpkgs_name)
                    .expect("Couldn't find `nixpkgs` in `root.inputs`!")),
                InputValue::List(_) => Err(FlakeCheckError::FoundListInRoot),
            }
        } else {
            Err(FlakeCheckError::NixpkgsNotFound)
        }
    }

    pub fn check(&self, allowed_age: u32) -> Result<FlakeStatus, FlakeCheckError> {
        let nixpkgs = self.lookup_nixpkgs()?;
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
            last_update_ts.expect("Couldn't find or parse last modified time for `nixpkgs` input."),
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
    }
}
