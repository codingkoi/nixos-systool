// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

/// Top level configuration
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub notifications: NotificationsConfig,
    pub system_check: SystemCheckConfig,
    pub external_commands: ExternalCommandsConfig,
}

/// Configuration for notifications for long running commands
#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationsConfig {
    /// How long (in seconds) successful command notifications should be shown
    pub success_timeout: u32,
    /// How long (in seconds) failed command notifications should be shown
    pub failure_timeout: u32,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            success_timeout: 10,
            failure_timeout: 60,
        }
    }
}

/// Configuration for the system check command i.e. `check`
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemCheckConfig {
    /// How many days until the nixpkgs version is considered out of date.
    pub allowed_age: u32,
}

impl Default for SystemCheckConfig {
    fn default() -> Self {
        Self {
            allowed_age: 14, // days
        }
    }
}

/// Configuration for external command paths
#[derive(Debug, Serialize, Deserialize)]
pub struct ExternalCommandsConfig {
    /// Command to open a browser
    pub browser_open: String,
    /// Path to the Git binary
    pub git: String,
    /// Path to the Manix binary
    pub manix: String,
}

impl Default for ExternalCommandsConfig {
    #[cfg(target_os = "linux")]
    fn default() -> Self {
        Self {
            browser_open: "xdg-open".to_owned(),
            git: "git".to_owned(),
            manix: "manix".to_owned(),
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn default() -> Self {
        Self {
            browser_open: "open".to_owned(),
            git: "git".to_owned(),
            manix: "manix".to_owned(),
        }
    }
}
