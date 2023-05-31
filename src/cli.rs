// SPDX-License-Identifier: GPL-3.0-or-later
//! Module for CLI option handling
use std::fmt::{Display, Formatter};

use anyhow::Context;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use duct::cmd;
use serde::{Deserialize, Serialize};

use crate::{config::Config, errors::SystoolError, excursion::Directory};

/// This struct combines the two sources of configuration into
/// a flattend structure
#[derive(Debug, Serialize, Deserialize)]
pub struct CliConfig {
    #[serde(flatten)]
    pub config_file: Config,
    #[serde(flatten)]
    pub cli: Cli,
}

#[derive(Debug, Parser, Serialize, Deserialize)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
    /// Path to the system configuration flake repository
    #[arg(short, long, env = "SYS_FLAKE_PATH")]
    pub flake_path: String,
    /// Path to the current system flake in the Nix store
    #[arg(short, long, default_value = "/etc/current-system-flake")]
    pub current_flake_path: String,
}

/// NixOS system management tool
#[derive(Debug, Subcommand, Clone, Serialize, Deserialize)]
pub enum Commands {
    /// Apply the system configuration using nixos-rebuild
    Apply {
        /// Method used to apply the system configuration
        ///
        /// Must be a valid build type accepted by `nixos-rebuild`, e.g.
        /// switch, boot, build, etc.
        method: Option<String>,
    },
    /// Apply user configuration using home-manager
    ApplyUser {
        /// User configuration to apply, defaults to the
        /// current user.
        #[arg(short = 'u', long = "user")]
        target_user: Option<String>,
    },
    /// Run garbage collection on the Nix store
    Clean,
    /// Build the system configuration, without applying it
    Build {
        /// Which system to build, defaults to the current host
        system: Option<String>,
        /// Whether to build a VM image instead
        #[arg(long)]
        vm: bool,
    },
    /// Prune old generations from the Nix store
    Prune,
    /// Search Nixpkgs or NixOS options
    Search {
        /// Pattern to search for in Nixpkgs
        query: String,
        /// Search on the NixOS website in a browser
        #[arg(short, long)]
        browser: bool,
        /// Search for options instead of packages
        #[arg(short, long)]
        options: bool,
        /// Search on the Home Manager option search website in a browser.
        /// Implies the `-b` option because there is no CLI version. Use
        /// regular options `-o` search for that.
        #[arg(short = 'm', long)]
        home_manager: bool,
    },
    /// Update the system flake lock
    Update,
    /// Check if the flake lock is outdated
    Check {
        /// Suppress the warning about using the repository flake.lock for
        /// the version check instead of the flake.lock used to build the system.
        #[arg(long)]
        no_warning: bool,
    },
    /// Print the currently loaded configuration including defaults
    PrintConfig,
}

impl Display for Commands {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let display = match self {
            Commands::Apply { .. } => "apply",
            Commands::Build { .. } => "build",
            Commands::ApplyUser { .. } => "apply-user",
            Commands::Clean => "clean",
            Commands::Prune => "prune",
            Commands::Search { .. } => "search",
            Commands::Update => "update",
            Commands::Check { .. } => "check",
            Commands::PrintConfig => "print-config",
        };
        f.write_str(display)
    }
}

impl Commands {
    /// Returns true if the command should send a DBus-style notification
    /// on successful completion.
    pub fn should_notify(&self) -> bool {
        !matches!(
            self,
            Commands::Search { .. }
                | Commands::Update
                | Commands::Check { .. }
                | Commands::PrintConfig
        )
    }

    /// Checks for any untracked files in the system flake and reports an
    /// error if there are. Usually this is something that will cause confusion
    /// if it's allowed to slip by.
    pub fn check_untracked_files(
        &self,
        flake_path: &Utf8PathBuf,
        cfg: &Config,
    ) -> anyhow::Result<()> {
        if matches!(
            self,
            Commands::Search { .. }
                | Commands::Update
                | Commands::Check { .. }
                | Commands::PrintConfig
        ) {
            return Ok(());
        }

        let _dir = Directory::enter(flake_path)
            .with_context(|| format!("Failed to enter flake path {flake_path}"))?;

        let status = match cmd!(&cfg.external_commands.git, "status", "--short")
            .stderr_null()
            .read()
        {
            Ok(s) => s,
            // If we get an error here, it's probably because we're not in a
            // Git repo, which means we don't care about untracked files.
            Err(_) => return Ok(()),
        };

        let untracked = status
            .lines()
            .filter(|l| l.starts_with("??"))
            .map(|l| {
                l.strip_prefix("?? ")
                    .expect("Couldn't strip prefix.")
                    .to_owned()
            })
            .collect::<Vec<String>>();

        if untracked.is_empty() {
            Ok(())
        } else {
            Err(SystoolError::UntrackedFiles(untracked.join("\n")).into())
        }
    }

    // Check to see if this command is valid to run on this system.
    // Currently this means whether or not the command can be run on a
    // non-NixOS system, e.g. on a system with just `nix` installed.
    pub fn valid_on_system(&self) -> anyhow::Result<()> {
        match self {
            Commands::Apply { .. } => {
                let info = os_info::get();
                match info.os_type() {
                    os_info::Type::NixOS => Ok(()),
                    _ => Err(SystoolError::NonNixOsSystem(self.clone(), info.os_type()).into()),
                }
            }
            _ => Ok(()),
        }
    }
}
