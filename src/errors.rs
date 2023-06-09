// SPDX-License-Identifier: GPL-3.0-or-later

use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum SystoolError {
    #[error("Cannot `{0}` on {1} systems")]
    NonNixOsSystem(String, os_info::Type),
    #[error("Untracked files in flake: \n{0}")]
    UntrackedFiles(String),
    #[error("Invalid options: {0}")]
    InvalidOptions(String),
}
