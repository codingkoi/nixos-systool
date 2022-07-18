mod flake_lock;
mod messages;

use crate::flake_lock::{FlakeLock, FlakeStatus};

use clap::{Parser, Subcommand};
use duct::cmd;
use nix::unistd::Uid;
use notify_rust::{Hint, Notification, Timeout, Urgency};
use owo_colors::OwoColorize;
use std::env::{current_dir, set_current_dir};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::process::exit;

/// How long the success notification should be displayed before disappearing
const SUCCESS_TIMEOUT: Timeout = Timeout::Milliseconds(10_000);
/// How long the failure notification should be displayed before disappearing
const FAILURE_TIMEOUT: Timeout = Timeout::Milliseconds(60_000);

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

/// NixOS system management tool
#[derive(Debug, Subcommand)]
enum Commands {
    /// Apply the system configuration using nixos-apply
    Apply {
        /// Method used to apply the system configuration
        ///
        /// Must be a valid build type accepted by `nixos-rebuild`.
        #[clap(value_parser)]
        method: Option<String>,
    },
    /// Apply user configuration using home-manager
    ApplyUser {
        /// Path to the system configuration flake
        #[clap(env = "SYS_FLAKE_PATH", value_parser)]
        flake_path: PathBuf,
        /// User configuration to apply, defaults to the
        /// current user.
        #[clap(short = 'u', long = "user", value_parser)]
        target_user: Option<String>,
    },
    /// Run garbage collection on the Nix store
    Clean,
    /// Prune old generations from the Nix store
    Prune,
    /// Search Nixpkgs or NixOS options
    Search {
        /// Pattern to search for in Nixpkgs
        #[clap(value_parser)]
        query: String,
        /// Search on the NixOS website in a browser
        #[clap(short, long, value_parser)]
        browser: bool,
        /// Search for options instead of packages
        #[clap(short, long, value_parser)]
        options: bool,
    },
    /// Update the system flake lock
    Update {
        /// Path to the system configuration flake
        #[clap(env = "SYS_FLAKE_PATH", value_parser)]
        flake_path: PathBuf,
    },
    /// Check if the flake lock is outdated
    Check {
        /// Path to the system configuration flake
        #[clap(env = "SYS_FLAKE_PATH", value_parser)]
        flake_path: PathBuf,
    },
}

impl Display for Commands {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let display = match self {
            Commands::Apply { .. } => "apply",
            Commands::ApplyUser { .. } => "apply-user",
            Commands::Clean => "clean",
            Commands::Prune => "prune",
            Commands::Search { .. } => "search",
            Commands::Update { .. } => "update",
            Commands::Check { .. } => "check",
        };
        f.write_str(display)
    }
}

impl Commands {
    /// Returns true if the command should send a DBus-style notification
    /// on successful completion.
    fn should_notify(&self) -> bool {
        !matches!(
            self,
            Commands::Search { .. } | Commands::Update { .. } | Commands::Check { .. }
        )
    }
}

fn main() {
    // For security reasons, I don't want this tool run as root, so check and exit
    // if that's the case.
    if Uid::effective().is_root() {
        error!("For security reasons, nixos-systool must not be run as root");
        exit(1);
    }

    let command = Cli::parse().command;
    if let Err(e) = run_command(&command) {
        error!("Error running command");
        error!(format!("  - {e}"));
        if command.should_notify() {
            Notification::new()
                .summary("NixOS System Tool")
                .body(
                    format!("`{command}` command execution failed.\nSee output for details")
                        .as_str(),
                )
                .appname("nixos-systool")
                .hint(Hint::Urgency(Urgency::Critical))
                .timeout(FAILURE_TIMEOUT)
                .show()
                .ok();
            exit(1);
        }
    };
    // Send a notification on success for commands that we want to notify on
    if command.should_notify() {
        Notification::new()
            .summary("NixOS System Tool")
            .body(format!("`{command}` command executed successfully").as_str())
            .appname("nixos-systool")
            .timeout(SUCCESS_TIMEOUT)
            .show()
            .ok();
    };
}

fn run_command(command: &Commands) -> Result<(), Box<dyn Error>> {
    match command {
        Commands::Apply { method } => {
            let method = match method {
                None => "switch".to_string(),
                Some(method) => method.to_string(),
            };
            info!("Applying system configuration");
            // Use `--use-remote-sudo` flag because Git won't recognize the
            // system flake repository when run using `sudo` due to a CVE fix.
            cmd!("nixos-rebuild", "--use-remote-sudo", method).run()?;
        }
        Commands::ApplyUser {
            flake_path,
            target_user,
        } => {
            let flake_path = flake_path
                .as_os_str()
                .to_str()
                .expect("Couldn't convert flake path to string!");
            let user = match target_user {
                Some(user) => user.to_owned(),
                None => cmd!("whoami").read()?,
            };
            info!(format!("Applying user settings for '{user}'"));
            cmd!(
                "home-manager",
                "switch",
                "--flake",
                format!("{flake_path}#{user}"),
            )
            .run()?;
        }
        Commands::Clean => {
            info!("Running garbage collection");
            cmd!("nix", "store", "gc").run()?;
            info!("Deduplication running... this may take a while");
            cmd!("nix", "store", "optimise").run()?;
        }
        Commands::Prune => {
            info!("Pruning old generations");
            cmd!("sudo", "nix-collect-garbage", "-d").run()?;
        }
        Commands::Search {
            query,
            browser,
            options,
        } => {
            if *options {
                info!(format!("Searching options for '{query}'"));
                if *browser {
                    cmd!(
                        "xdg-open",
                        format!("https://search.nixos.org/options?channel=unstable&query={query}")
                    )
                    .run()?;
                } else {
                    cmd!("manix", query).run()?;
                }
            } else {
                info!(format!("Searching nixpkgs for '{query}'"));
                if *browser {
                    cmd!(
                        "xdg-open",
                        format!("https://search.nixos.org/packages?channel=unstable&query={query}")
                    )
                    .run()?;
                } else {
                    cmd!("nix", "search", "nixpkgs", query).run()?;
                }
            }
        }
        Commands::Update { flake_path } => {
            let pwd = current_dir()?;
            set_current_dir(flake_path)?;

            info!("Updating system configuration flake");
            cmd!("nix", "flake", "update").run()?;
            // commit changes
            cmd!("git", "add", "flake.lock").run()?;
            cmd!("git", "commit", "-m", "Update flake lock").run()?;
            set_current_dir(pwd)?;
        }
        Commands::Check { flake_path } => {
            let mut flake_lock_filename = flake_path.clone();
            flake_lock_filename.push("flake");
            flake_lock_filename.set_extension("lock");
            let check_result = FlakeLock::load(flake_lock_filename)?.check()?;
            match check_result {
                FlakeStatus::UpToDate { last_update } => {
                    info!("System flake lock is up to date.");
                    info!(format!("  Last updated on {last_update}"));
                }
                FlakeStatus::Outdated { last_update } => {
                    error!(format!(
                        "System flake lock has been out of date since {last_update}"
                    ));
                    error!("Please update as soon as possible using `nixos-systool update`.");
                }
            }
        }
    }
    Ok(())
}
