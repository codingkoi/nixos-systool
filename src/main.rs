// SPDX-License-Identifier: GPL-3.0-or-later

use camino::Utf8PathBuf;
use clap::Parser;
use directories::BaseDirs;
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use nix::unistd::Uid;
use nixos_systool::{
    cli::{Cli, CliConfig},
    config::Config,
    error, run_command, CRATE_NAME,
};
use notify_rust::{Notification, Timeout};
use owo_colors::OwoColorize;
use std::process::exit;

#[cfg(target_os = "linux")]
fn add_notification_hints(notification: &mut Notification) {
    use notify_rust::{Hint, Urgency};
    notification.hint(Hint::Urgency(Urgency::Critical));
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
            error!(format!("{e:#}"));
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
