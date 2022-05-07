use clap::{Parser, Subcommand};
use color_eyre::Result;
use duct::cmd;
use nix::unistd::Uid;
use nixos_systool::{FlakeLock, FlakeStatus};
use notify_rust::{Hint, Notification, Timeout, Urgency};
use owo_colors::OwoColorize;
use std::env::{current_dir, set_current_dir};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::process::exit;

/// How long the success notification should be displayed before disappearing
const NOTIFICATION_TIMEOUT: Timeout = Timeout::Milliseconds(10_000);

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
        method: Option<String>,
    },
    /// Apply user configuration using home-manager
    ApplyUser {
        /// Path to the system configuration flake
        #[clap(env = "SYS_FLAKE_PATH")]
        flake_path: PathBuf,
    },
    /// Run garbage collection on the Nix store
    Clean,
    /// Prune old generations from the Nix store
    Prune,
    /// Search nixpkgs
    Search {
        /// Pattern to search for in nixpkgs
        query: String,
        /// Search on the NixOS website in a browser
        #[clap(short, long)]
        browser: bool,
        /// Search for options instead of packages
        #[clap(short, long)]
        options: bool,
    },
    /// Update the system flake lock
    Update {
        /// Path to the system configuration flake
        #[clap(env = "SYS_FLAKE_PATH")]
        flake_path: PathBuf,
    },
    /// Check if the flake lock is outdated
    Check {
        /// Path to the system configuration flake
        #[clap(env = "SYS_FLAKE_PATH")]
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
    // that's the case.
    if Uid::effective().is_root() {
        eprintln!(
            "{}",
            "For security reasons, nixos-systool must not be run as root"
                .red()
                .italic()
        );
        exit(1);
    }

    let command = Cli::parse().command;
    if let Err(e) = run_command(&command) {
        eprintln!("{}", "Error running command".yellow().italic());
        eprintln!("  - {e}");
        Notification::new()
            .summary("NixOS System Tool")
            .body(format!("`{command}` command execution failed.\nSee output for details").as_str())
            .appname("nixos-systool")
            .hint(Hint::Urgency(Urgency::Critical))
            .timeout(Timeout::Never)
            .show()
            .expect("Failed to show notification");
        exit(1);
    };
    // Send a notification on success for commands that we want to notify on
    if command.should_notify() {
        Notification::new()
            .summary("NixOS System Tool")
            .body(format!("`{command}` command executed successfully").as_str())
            .appname("nixos-systool")
            .timeout(NOTIFICATION_TIMEOUT)
            .show()
            .expect("Failed to show notification");
    };
}

fn run_command(command: &Commands) -> Result<()> {
    match command {
        Commands::Apply { method } => {
            let method = match method {
                None => "switch".to_string(),
                Some(method) => method.to_string(),
            };
            println!("{}", "Applying system configuration".italic());
            // Use ``--use-remote-sudo` flag because Git won't recognize the
            // system flake repository when run using `sudo` due to a CVE fix.
            cmd!("nixos-rebuild", "--use-remote-sudo", method).run()?;
        }
        Commands::ApplyUser { flake_path } => {
            let pwd = current_dir()?;
            set_current_dir(flake_path)?;

            let user = cmd!("whoami").read()?;
            println!("{}", "Applying user settings".italic());
            cmd!(
                "nix",
                "build",
                format!(".#homeConfigurations.{user}.activationPackage"),
            )
            .run()?;
            cmd!("./result/activate").run()?;
            cmd!("rm", "./result").run()?;
            set_current_dir(pwd)?;
        }
        Commands::Clean => {
            println!("{}", "Running garbage collection".italic());
            cmd!("nix", "store", "gc").run()?;
            println!(
                "{}",
                "Deduplication running... this may take a while".italic()
            );
            cmd!("nix", "store", "optimise").run()?;
        }
        Commands::Prune => {
            println!("{}", "Pruning old generations".italic());
            cmd!("sudo", "nix-collect-garbage", "-d").run()?;
        }
        Commands::Search {
            query,
            browser,
            options,
        } => {
            if *options {
                println!("{}", format!("Searching options for '{query}'").italic());
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
                println!("{}", format!("Searching nixpkgs for '{query}'").italic());
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

            println!("{}", "Updating system configuration flake".italic());
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
                FlakeStatus::UpToDate => {
                    println!("{}", "System flake lock is up to date.".italic());
                }
                FlakeStatus::Outdated { since } => {
                    println!(
                        "{}",
                        format!("System flake lock has been out of date since {since}").red()
                    );
                    println!(
                        "{}",
                        "Please update as soon as possible using `nixos-systool update`.".red()
                    );
                }
            }
        }
    }
    Ok(())
}
