use serde::{Deserialize, Serialize};

/// Top level configuration
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub notifications: NotificationsConfig,
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
    fn default() -> Self {
        Self {
            browser_open: "xdg-open".to_owned(),
            git: "git".to_owned(),
            manix: "manix".to_owned(),
        }
    }
}
