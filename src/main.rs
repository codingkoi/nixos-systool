// SPDX-License-Identifier: GPL-3.0-or-later

use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, Subcommand};
use directories::BaseDirs;
use duct::cmd;
use figment::providers::{Format, Serialized, Toml};
use figment::Figment;
use nix::unistd::Uid;
use nixos_systool::config::Config;
use nixos_systool::excursion::Directory;
use nixos_systool::flake_lock::{FlakeLock, FlakeStatus};
use nixos_systool::{error, info, warn};
use notify_rust::{Notification, Timeout};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::process::exit;
use thiserror::Error as ThisError;

const CRATE_NAME: &str = clap::crate_name!();

#[derive(Debug, ThisError)]
enum SystoolError {
    #[error("Cannot `{0}` on {1} systems")]
    NonNixOsSystem(Commands, os_info::Type),
    #[error("Untracked files in flake: \n{0}")]
    UntrackedFiles(String),
    #[error("Invalid options: {0}")]
    InvalidOptions(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct CliConfig {
    #[serde(flatten)]
    config_file: Config,
    #[serde(flatten)]
    cli: Cli,
}

#[derive(Debug, Parser, Serialize, Deserialize)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
    /// Path to the system configuration flake repository
    #[arg(short, long, env = "SYS_FLAKE_PATH")]
    flake_path: String,
    /// Path to the current system flake in the Nix store
    #[arg(short, long, default_value = "/etc/current-system-flake")]
    current_flake_path: String,
}

/// NixOS system management tool
#[derive(Debug, Subcommand, Clone, Serialize, Deserialize)]
enum Commands {
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
    fn should_notify(&self) -> bool {
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
    fn check_untracked_files(
        &self,
        flake_path: &Utf8PathBuf,
        cfg: &Config,
    ) -> Result<(), Box<dyn Error>> {
        if matches!(
            self,
            Commands::Search { .. }
                | Commands::Update
                | Commands::Check { .. }
                | Commands::PrintConfig
        ) {
            return Ok(());
        }

        let _dir = Directory::enter(flake_path)?;

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
    fn valid_on_system(&self) -> Result<(), Box<dyn Error>> {
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

#[cfg(target_os = "linux")]
fn add_notification_hints(notification: &mut Notification) {
    use notify_rust::{Hint, Urgency};
    notification.hint(Hint::Urgency(Urgency::Critical))
}

// No-op for non Linux hosts
#[cfg(not(target_os = "linux"))]
fn add_notification_hints(_notification: &mut Notification) {}

fn main() {
    // For security reasons, I don't want this tool run as root, so check and exit
    // if that's the case.
    if Uid::effective().is_root() {
        error!(format!(
            "For security reasons, {CRATE_NAME} must not be run as root"
        ));
        exit(1);
    }

    // Create a Figment for merging configuration sources and load the defaults
    let mut fig = Figment::new().merge(Serialized::defaults(Config::default()));
    // Load the user configuration if we can find it
    if let Some(base_dirs) = BaseDirs::new() {
        let mut config_base = Utf8PathBuf::from_path_buf(base_dirs.config_dir().into())
            .expect("Couldn't parse config path.");
        config_base.push("nixos-systool");
        config_base.push("config.toml");
        fig = fig.merge(Toml::file(config_base.as_str()));
    }
    // Add the command line options
    let config: CliConfig = fig
        .merge(Serialized::defaults(Cli::parse()))
        .extract()
        .unwrap_or_else(|e| {
            error!("Error loading configuration");
            error!(format!("- {e}"));
            exit(1);
        });

    let command = config.cli.command;
    let cfg = config.config_file;
    if let Err(e) = run_command(&command, &config.cli.flake_path.into(), &cfg) {
        error!("Error running command");
        error!(format!("- {e}"));
        if command.should_notify() {
            let mut notification = Notification::new();
            notification
                .summary("NixOS System Tool")
                .body(
                    format!("`{command}` command execution failed.\nSee output for details")
                        .as_str(),
                )
                .appname(CRATE_NAME)
                .timeout(Timeout::Milliseconds(
                    cfg.notifications.failure_timeout * 1000,
                ));
            add_notification_hints(&mut notification);
            notification.show().ok();
        }
    };
    // Send a notification on success for commands that we want to notify on
    if command.should_notify() {
        Notification::new()
            .summary("NixOS System Tool")
            .body(format!("`{command}` command executed successfully").as_str())
            .appname(CRATE_NAME)
            .timeout(Timeout::Milliseconds(
                cfg.notifications.success_timeout * 1000,
            ))
            .show()
            .ok();
    };
}

fn run_command(
    command: &Commands,
    flake_path: &Utf8PathBuf,
    cfg: &Config,
) -> Result<(), Box<dyn Error>> {
    // Check for untracked files if we need to
    command.check_untracked_files(flake_path, cfg)?;
    // Check if this command can be run on this system
    command.valid_on_system()?;

    match command {
        Commands::Apply { method } => {
            let method = match method {
                None => "switch".to_string(),
                Some(method) => method.to_string(),
            };
            info!("Applying system configuration");
            cmd!(
                "nixos-rebuild",
                // Use `--use-remote-sudo` flag because Git won't recognize the
                // system flake repository when run using `sudo` due to a CVE fix.
                "--use-remote-sudo",
                // Don't assume that /etc/nixos/flake.nix exists, just specify the
                // flake path directly.
                "--flake",
                flake_path,
                method
            )
            .run()?;
        }
        Commands::ApplyUser { target_user } => {
            let flake_path = flake_path.as_str();
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
        Commands::Build { system, vm } => {
            let system = match system {
                Some(s) => s.to_owned(),
                None => cmd!("hostname").read()?,
            };

            let _dir = Directory::enter(flake_path)?;

            let flake_path = flake_path.as_str();
            info!(format!("Building system configuration for {system}"));
            let build_type = match vm {
                true => "vm",
                false => "toplevel",
            };
            cmd!(
                "nix",
                "build",
                format!(".#nixosConfigurations.{system}.config.system.build.{build_type}")
            )
            .run()?;
            match vm {
                true => info!(format!(
                    "VM image built. Run {flake_path}/result/bin/run-{system}-vm to start it."
                )),
                false => info!(format!("System built and symlinked to {flake_path}/result")),
            }
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
            home_manager,
        } => {
            // first check if we're doing something wrong
            if *home_manager && (*options || *browser) {
                return Err(SystoolError::InvalidOptions(
                    "cannot use --home-manager with other options".to_owned(),
                )
                .into());
            }
            // If we're doing a home-manager search, then use the browser
            if *home_manager {
                info!(format!("Searching home-manager for `{query}`"));
                cmd!(
                    &cfg.external_commands.browser_open,
                    format!("https://mipmip.github.io/home-manager-option-search/?{query}")
                )
                .run()?;
            } else if *options {
                // If we're searching for options, use `manix` or a browser
                info!(format!("Searching options for '{query}'"));
                if *browser {
                    cmd!(
                        &cfg.external_commands.browser_open,
                        format!("https://search.nixos.org/options?channel=unstable&query={query}")
                    )
                    .run()?;
                } else {
                    cmd!(&cfg.external_commands.manix, query).run()?;
                }
            } else {
                // Otherwise search for packages in Nixpkgs
                info!(format!("Searching nixpkgs for '{query}'"));
                if *browser {
                    cmd!(
                        &cfg.external_commands.browser_open,
                        format!("https://search.nixos.org/packages?channel=unstable&query={query}")
                    )
                    .run()?;
                } else {
                    cmd!("nix", "search", "nixpkgs", query).run()?;
                }
            }
        }
        Commands::Update => {
            let _dir = Directory::enter(flake_path)?;
            info!("Updating system configuration flake");
            cmd!("nix", "flake", "update").run()?;
            // commit changes
            cmd!(&cfg.external_commands.git, "add", "flake.lock").run()?;
            cmd!(
                &cfg.external_commands.git,
                "commit",
                "-m",
                "Update flake lock"
            )
            .run()?;
        }
        Commands::Check { no_warning } => {
            // If we have a link to the current system flake in /etc/current-system-flake
            // then use it for the check, otherwise, fallback to the less accurate
            // check of the flake repo path.
            let current_system_flake = Utf8Path::new("/etc/current-system-flake");
            let mut flake_lock_filename = match current_system_flake.exists() {
                true => current_system_flake.into(),
                false => {
                    if !no_warning {
                        warn!(format!(
                            "The flake in the the repository may not be applied to the system. \
                             Make sure to use `{CRATE_NAME} apply` or create a symlink in \
                             /etc/current-system-flake pointing to the source of the flake in \
                             the Nix store used to build the current system for a more accurate \
                             version check."
                        ));

                        warn!("\nAdd the following to your nixosSystem configuration to do so:");
                        warn!("    environment.etc.\"current-system-flake\".source = inputs.self;");
                    };
                    flake_path.clone()
                }
            };
            // Add the path parts for the "flake.lock" file.
            flake_lock_filename.push("flake");
            flake_lock_filename.set_extension("lock");

            let check_result =
                FlakeLock::load(&flake_lock_filename)?.check(cfg.system_check.allowed_age)?;
            match check_result {
                FlakeStatus::UpToDate { last_update, since } => {
                    let days_ago = since.num_days();
                    info!(format!(
                        "System flake ({flake_lock_filename}) is up to date."
                    ));
                    info!(format!(
                        "Last updated on {last_update} ({days_ago} days ago)"
                    ));
                }
                FlakeStatus::Outdated { last_update, since } => {
                    let days_ago = since.num_days();
                    error!(format!(
                        "System flake ({flake_lock_filename}) is out of date, last update was on {last_update} ({days_ago} days ago)"
                    ));
                    error!(format!("Please update as soon as possible using `{CRATE_NAME} update` and `{CRATE_NAME} apply`."));
                }
            }
        }
        Commands::PrintConfig => {
            let rendered_config =
                toml::to_string(&cfg).expect("Couldn't render configuration to TOML!");
            println!("{rendered_config}")
        }
    }
    Ok(())
}
