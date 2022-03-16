use color_eyre::Result;
use duct::cmd;
use notify_rust::{Hint, Notification, Timeout, Urgency};
use owo_colors::OwoColorize;
use std::env::{current_dir, set_current_dir};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::process::exit;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
/// NixOS system management tool
enum Commands {
    /// Apply the system configuration using nixos-apply
    Apply {
        /// Method used to apply the system configuration
        ///
        /// Must be one of "switch" (default), "boot", or "remote".
        method: Option<String>,
    },
    /// Apply user configuration using home-manager
    ApplyUser {
        /// Path to the system configuration flake
        #[structopt(env = "SYS_FLAKE_PATH")]
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
    },
    /// Update the system flake lock
    Update {
        /// Path to the system configuration flake
        #[structopt(env = "SYS_FLAKE_PATH")]
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
        };
        f.write_str(display)
    }
}

impl Commands {
    /// Returns true if the command should send a DBus-style notification
    /// on successful completion.
    fn should_notify(&self) -> bool {
        !matches!(self, Commands::Search { .. } | Commands::Update { .. })
    }
}

fn main() {
    let command = Commands::from_args();
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
            .timeout(Timeout::Never)
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
            cmd!("sudo", "nixos-rebuild", method, "--builders", "").run()?;
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
                "--builders",
                ""
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
        Commands::Search { query } => {
            println!("{}", format!("Searching nixpkgs for '{query}'").italic());
            cmd!("nix", "search", "nixpkgs", query).run()?;
        }
        Commands::Update { flake_path } => {
            let pwd = current_dir()?;
            set_current_dir(flake_path)?;

            println!("{}", "Updating system configuration flake".italic());
            cmd!("nix", "flake", "update").run()?;
            // TODO - add commands to auto-commit lock update

            set_current_dir(pwd)?;
        }
    }
    Ok(())
}
