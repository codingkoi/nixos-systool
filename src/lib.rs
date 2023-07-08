// SPDX-License-Identifier: GPL-3.0-or-later

pub mod cli;
pub mod commands;
pub mod config;
pub mod errors;
pub mod excursion;
pub mod flake_lock;
pub mod messages;

use anyhow::Result;
use camino::Utf8PathBuf;
use cli::Commands;
use config::Config;
use duct::cmd;
use owo_colors::OwoColorize;

pub const CRATE_NAME: &str = clap::crate_name!();

/// Runs the specified command, routing it to the appropriate command function
pub fn run_command(command: &Commands, flake_path: &Utf8PathBuf, cfg: &Config) -> Result<()> {
    // Check for untracked files if we need to
    command.check_untracked_files(flake_path, cfg)?;

    match command {
        Commands::Apply { method } => commands::apply(method, flake_path),
        Commands::ApplyUser { target_user } => commands::apply_user(target_user, flake_path),
        Commands::Build { system, vm } => commands::build_system(system, *vm, flake_path),
        Commands::Clean => {
            info!("Running garbage collection");
            cmd!("nix", "store", "gc").run()?;
            info!("Deduplication running... this may take a while");
            cmd!("nix", "store", "optimise").run()?;
            Ok(())
        }
        Commands::Prune => {
            info!("Pruning old generations");
            cmd!("sudo", "nix-collect-garbage", "-d").run()?;
            Ok(())
        }
        Commands::Search {
            query,
            browser,
            options,
            home_manager,
        } => commands::search(query, *browser, *options, *home_manager, cfg),
        Commands::Update => commands::update_flake(flake_path, cfg),
        Commands::Check { no_warning } => {
            commands::check_flake_version(*no_warning, flake_path, cfg)
        }
        Commands::PrintConfig => {
            let rendered_config =
                toml::to_string(&cfg).expect("Couldn't render configuration to TOML!");
            println!("{rendered_config}");
            Ok(())
        }
    }
}
