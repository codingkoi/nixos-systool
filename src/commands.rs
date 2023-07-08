// SPDX-License-Identifier: GPL-3.0-or-later

//! Module containing the individual subcommands that the tool can run
use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use duct::cmd;
use owo_colors::OwoColorize;

use crate::{
    config::Config,
    error,
    errors::SystoolError,
    excursion::Directory,
    flake_lock::{FlakeLock, FlakeStatus},
    info, warn, CRATE_NAME,
};

pub fn apply(method: &Option<String>, flake_path: &Utf8PathBuf) -> Result<()> {
    let method = match method {
        None => "switch".to_string(),
        Some(method) => method.to_string(),
    };

    // Check to see if this command is valid to run on this system.
    // Currently this means whether or not the command can be run on a
    // non-NixOS system, e.g. on a system with just `nix` installed.
    let info = os_info::get();
    match info.os_type() {
        // For NixOS systems use `nixos-rebuild`
        os_info::Type::NixOS => {
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
            Ok(())
        }
        // For MacOS systems try to use `darwin-rebuild`
        os_info::Type::Macos => {
            info!("Applying system configuration");
            cmd!("darwin-rebuild", "--flake", flake_path, method).run()?;
            Ok(())
        }
        _ => Err(SystoolError::NonNixOsSystem("apply".to_string(), info.os_type()).into()),
    }
}

pub fn apply_user(target_user: &Option<String>, flake_path: &Utf8PathBuf) -> Result<()> {
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
    Ok(())
}

pub fn build_system(system: &Option<String>, vm: bool, flake_path: &Utf8PathBuf) -> Result<()> {
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
    };
    Ok(())
}

pub fn search(
    query: &str,
    browser: bool,
    options: bool,
    home_manager: bool,
    cfg: &Config,
) -> Result<()> {
    // first check if we're doing something wrong
    if home_manager && (options || browser) {
        return Err(SystoolError::InvalidOptions(
            "cannot use --home-manager with other options".to_owned(),
        )
        .into());
    }
    // If we're doing a home-manager search, then use the browser
    if home_manager {
        info!(format!("Searching home-manager for `{query}`"));
        cmd!(
            &cfg.external_commands.browser_open,
            format!("https://mipmip.github.io/home-manager-option-search/?{query}")
        )
        .run()?;
    } else if options {
        // If we're searching for options, use `manix` or a browser
        info!(format!("Searching options for '{query}'"));
        if browser {
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
        if browser {
            cmd!(
                &cfg.external_commands.browser_open,
                format!("https://search.nixos.org/packages?channel=unstable&query={query}")
            )
            .run()?;
        } else {
            cmd!("nix", "search", "nixpkgs", query).run()?;
        }
    }
    Ok(())
}

pub fn update_flake(flake_path: &Utf8PathBuf, cfg: &Config) -> Result<()> {
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
    Ok(())
}

pub fn check_flake_version(no_warning: bool, flake_path: &Utf8PathBuf, cfg: &Config) -> Result<()> {
    let wrap_options = textwrap::Options::with_termwidth();

    // If we have a link to the current system flake in the nix store
    // then use it for the check, otherwise, fallback to the less accurate
    // check of the flake repo path.
    let current_flake_path = Utf8Path::new(&cfg.system_check.current_system_flake_path);
    let current_flake_status = if current_flake_path.exists() {
        let mut path: Utf8PathBuf = current_flake_path.into();
        path.push("flake");
        path.set_extension("lock");
        Some(FlakeLock::load(&path)?.check(cfg.system_check.allowed_age)?)
    } else {
        None
    };

    // Get the status of the config flake
    let mut path = flake_path.clone();
    path.push("flake");
    path.set_extension("lock");
    let config_flake_status = FlakeLock::load(&path)?.check(cfg.system_check.allowed_age)?;

    if let Some(current_status) = current_flake_status {
        match current_status {
            FlakeStatus::UpToDate { last_update, since } => {
                let days_ago = since.num_days();
                let last_update_str = last_update.format(&cfg.system_check.date_format);
                let msg = format!("System flake is up to date. Last updated on {last_update_str} ({days_ago} days ago)");
                info!(textwrap::fill(&msg, &wrap_options));
                if config_flake_status.last_update() > &last_update {
                    let last_update_str = config_flake_status
                        .last_update()
                        .format(&cfg.system_check.date_format);
                    let msg = format!(
                        "Config flake is AHEAD of the current system flake, last updated on {last_update_str}. \
                         Consider running `{CRATE_NAME} apply`."
                    );
                    warn!(textwrap::fill(&msg, &wrap_options));
                }
            }
            FlakeStatus::Outdated { last_update, since } => {
                let days_ago = since.num_days();
                let last_update_str = last_update.format(&cfg.system_check.date_format);
                let msg = format!(
                        "System flake is out of date, last update was on {last_update_str} ({days_ago} days ago)"
                    );
                error!(textwrap::fill(&msg, &wrap_options));
                match config_flake_status {
                    FlakeStatus::UpToDate { last_update, since } => {
                        let days_ago = since.num_days();
                        let last_update_str = last_update.format(&cfg.system_check.date_format);
                        let msg = format!(
                            "Config flake is up to date, last updated on {last_update_str} ({days_ago} days ago). \
                             Update the system flake to use this one using `{CRATE_NAME} apply`."
                        );
                        warn!(textwrap::fill(&msg, &wrap_options));
                    }
                    FlakeStatus::Outdated { .. } => {
                        let msg = format!(
                            "Please update as soon as possible using `{CRATE_NAME} update` \
                             and `{CRATE_NAME} apply`."
                        );
                        error!(textwrap::fill(&msg, &wrap_options));
                    }
                }
            }
        }
    } else {
        // We don't have a link to the current system flake, so do the best we can without it
        if !no_warning {
            let msg = format!(
                "The flake in the the repository may not be applied to the system. \
                 Make sure to use `{CRATE_NAME} apply` or create a symlink in \
                 /etc/current-system-flake pointing to the source of the flake in \
                 the Nix store used to build the current system for a more accurate \
                 version check."
            );
            warn!(textwrap::fill(&msg, &wrap_options));

            warn!("\nAdd the following to your nixosSystem configuration to do so:");
            warn!("    environment.etc.\"current-system-flake\".source = inputs.self;");
        };
        match config_flake_status {
            FlakeStatus::UpToDate { last_update, since } => {
                let days_ago = since.num_days();
                let last_update_str = last_update.format(&cfg.system_check.date_format);
                let msg = format!(
                    "Config flake is up to date, last updated on {last_update_str} ({days_ago} days ago). \
                     Update the system flake to use this one using `{CRATE_NAME} apply`."
                );
                warn!(textwrap::fill(&msg, &wrap_options));
            }
            FlakeStatus::Outdated { .. } => {
                let msg = format!(
                    "Please update as soon as possible using `{CRATE_NAME} update` \
                     and `{CRATE_NAME} apply`."
                );
                error!(textwrap::fill(&msg, &wrap_options));
            }
        }
    }
    Ok(())
}
