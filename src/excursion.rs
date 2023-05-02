// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use std::{
    env::{current_dir, set_current_dir},
    path::{Path, PathBuf},
};

/// Utility to handle changing directories and returning after finishing
#[derive(Debug)]
pub struct Directory {
    previous_dir: PathBuf,
}

impl Directory {
    /// Enter the specified directory, keeping track of the previous
    /// directory. Returns a `Directory` object, that when dropped will
    /// change back to the previous directory.
    pub fn enter<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let previous_dir = current_dir()?;
        set_current_dir(dir.as_ref())?;
        Ok(Directory { previous_dir })
    }
}

impl Drop for Directory {
    /// When dropped, try to change back to the directory we stored before
    /// we entered this directory.
    fn drop(&mut self) {
        set_current_dir(&self.previous_dir).unwrap_or_else(|_| {
            panic!(
                "Couldn't return to previous directory: {:?}",
                self.previous_dir
            )
        });
    }
}
