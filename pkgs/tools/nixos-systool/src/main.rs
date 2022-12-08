use clap::{Parser, Subcommand};
use duct::cmd;
use nix::unistd::Uid;
use nixos_systool::excursion::Directory;
use nixos_systool::flake_lock::{FlakeLock, FlakeStatus};
use nixos_systool::{error, info};
use notify_rust::{Hint, Notification, Timeout, Urgency};
use owo_colors::OwoColorize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::process::exit;
use thiserror::Error as ThisError;

/// How long the success notification should be displayed before disappearing
const SUCCESS_TIMEOUT: Timeout = Timeout::Milliseconds(10_000);
/// How long the failure notification should be displayed before disappearing
const FAILURE_TIMEOUT: Timeout = Timeout::Milliseconds(60_000);

#[derive(Debug, ThisError)]
enum SystoolError {
    #[error("Cannot `{0}` on non-NixOS systems: {1}")]
    NonNixOsSystem(Commands, os_info::Type),
    #[error("Untracked files in flake: \n{0}")]
    UntrackedFiles(String),
    #[error("Invalid options: {0}")]
    InvalidOptions(String),
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
    /// Path to the system configuration flake
    #[arg(short, long, env = "SYS_FLAKE_PATH")]
    flake_path: PathBuf,
}

/// NixOS system management tool
#[derive(Debug, Subcommand, Clone)]
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
        browser: bool,
        /// Search for options instead of packages
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
    Check,
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
            Commands::Check => "check",
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
            Commands::Search { .. } | Commands::Update | Commands::Check
        )
    }

    /// Checks for any untracked files in the system flake and reports an
    /// error if there are. Usually this is something that will cause confusion
    /// if it's allowed to slip by.
    fn check_untracked_files(&self, flake_path: &PathBuf) -> Result<(), Box<dyn Error>> {
        if matches!(
            self,
            Commands::Search { .. } | Commands::Update | Commands::Check
        ) {
            return Ok(());
        }

        let _dir = Directory::enter(flake_path)?;

        let status = match cmd!("git", "status", "--short").stderr_null().read() {
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
                    os_info::Type::NixOS => {
                        Err(SystoolError::NonNixOsSystem(self.clone(), info.os_type()).into())
                    }
                    _ => Ok(()),
                }
            }
            _ => Ok(()),
        }
    }
}

fn main() {
    // For security reasons, I don't want this tool run as root, so check and exit
    // if that's the case.
    if Uid::effective().is_root() {
        error!("For security reasons, nixos-systool must not be run as root");
        exit(1);
    }

    let cli = Cli::parse();
    let command = cli.command;
    if let Err(e) = run_command(&command, &cli.flake_path) {
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

fn run_command(command: &Commands, flake_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    // Check for untracked files if we need to
    command.check_untracked_files(flake_path)?;
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
        Commands::Build { system, vm } => {
            let system = match system {
                Some(s) => s.to_owned(),
                None => cmd!("hostname").read()?,
            };

            let _dir = Directory::enter(flake_path)?;

            let flake_path = flake_path
                .as_os_str()
                .to_str()
                .expect("Couldn't convert flake path to string!");
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
                    "xdg-open",
                    format!("https://mipmip.github.io/home-manager-option-search/?{query}")
                )
                .run()?;
            } else if *options {
                // If we're searching for options, use `manix` or a browser
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
                // Otherwise search for packages in Nixpkgs
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
        Commands::Update => {
            let _dir = Directory::enter(flake_path)?;
            info!("Updating system configuration flake");
            cmd!("nix", "flake", "update").run()?;
            // commit changes
            cmd!("git", "add", "flake.lock").run()?;
            cmd!("git", "commit", "-m", "Update flake lock").run()?;
        }
        Commands::Check => {
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
