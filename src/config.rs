// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

/// Top level configuration
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub notifications: NotificationsConfig,
    pub system_check: SystemCheckConfig,
    pub external_commands: ExternalCommandsConfig,
    pub web_search: WebSearchConfig,
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
    /// Path to the flake that defines the current system
    pub current_system_flake_path: String,
    /// Date format string
    pub date_format: String,
}

impl Default for SystemCheckConfig {
    fn default() -> Self {
        Self {
            allowed_age: 14, // days
            current_system_flake_path: "/etc/current-system-flake".to_owned(),
            date_format: "%-e %B, %Y".to_owned(),
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

/// Configuration for web searches
#[derive(Debug, Serialize, Deserialize)]
pub struct WebSearchConfig {
    pub nixos_pkg_search: String,
    pub nixos_option_search: String,
    pub home_manager_search: String,
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            nixos_pkg_search: "https://search.nixos.org/packages?channel=unstable&query={}"
                .to_owned(),
            nixos_option_search: "https://search.nixos.org/options?channel=unstable&query={}"
                .to_owned(),
            home_manager_search: "https://mipmip.github.io/home-manager-option-search/?query={}"
                .to_owned(),
        }
    }
}
