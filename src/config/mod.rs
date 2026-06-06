//! Layered settings: embedded defaults, sidecar appsettings.json, environment overrides.

use crate::models::{AppSettings, SshOptions};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const EMBEDDED: &str = include_str!("../../appsettings.json");

/// Load configuration with precedence: embedded < sidecar file < environment.
pub fn load_settings(exe_dir: &Path) -> AppSettings {
    let mut settings = parse_json(EMBEDDED).unwrap_or_default();

    let sidecar = exe_dir.join("appsettings.json");
    if sidecar.is_file() {
        if let Ok(text) = fs::read_to_string(&sidecar) {
            if let Ok(file_settings) = parse_json(&text) {
                merge_settings(&mut settings, file_settings);
            }
        }
    }

    apply_env_overrides(&mut settings);
    settings
}

pub fn exe_directory() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn parse_json(text: &str) -> Result<AppSettings, serde_json::Error> {
    serde_json::from_str(text)
}

fn merge_settings(base: &mut AppSettings, overlay: AppSettings) {
    if !overlay.ssh.host.is_empty() {
        base.ssh.host = overlay.ssh.host;
    }
    if overlay.ssh.port != 0 {
        base.ssh.port = overlay.ssh.port;
    }
    if !overlay.ssh.username.is_empty() {
        base.ssh.username = overlay.ssh.username;
    }
    if overlay.ssh.password.is_some() {
        base.ssh.password = overlay.ssh.password;
    }
    if overlay.ssh.command_timeout_seconds != 0 {
        base.ssh.command_timeout_seconds = overlay.ssh.command_timeout_seconds;
    }
}

fn apply_env_overrides(settings: &mut AppSettings) {
    if let Ok(host) = env::var("Ssh__Host") {
        if !host.is_empty() {
            settings.ssh.host = host;
        }
    }
    if let Ok(port) = env::var("Ssh__Port") {
        if let Ok(p) = port.parse::<u16>() {
            settings.ssh.port = p;
        }
    }
    if let Ok(user) = env::var("Ssh__Username") {
        if !user.is_empty() {
            settings.ssh.username = user;
        }
    }
    if let Ok(pwd) = env::var("Ssh__Password") {
        settings.ssh.password = Some(pwd);
    }
}

impl SshOptions {
    pub fn effective_password<'a>(&'a self, ui_password: &'a str) -> Option<&'a str> {
        if !ui_password.is_empty() {
            Some(ui_password)
        } else {
            self.password.as_deref()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_defaults_parse() {
        let settings = parse_json(EMBEDDED).unwrap();
        assert_eq!(settings.ssh.port, 22);
        assert_eq!(settings.ssh.command_timeout_seconds, 120);
    }

    #[test]
    fn env_overrides_host() {
        let mut settings = AppSettings::default();
        env::set_var("Ssh__Host", "10.0.0.1");
        apply_env_overrides(&mut settings);
        env::remove_var("Ssh__Host");
        assert_eq!(settings.ssh.host, "10.0.0.1");
    }
}
