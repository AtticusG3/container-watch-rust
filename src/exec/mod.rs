//! Popout terminal for interactive `docker exec -it ... bash`.
//!
//! Uses the system OpenSSH client (`ssh.exe`) with a short-lived SSH_ASKPASS helper.

use crate::models::ConnectionDraft;
use crate::ssh_docker::escape_for_single_quoted_unix_shell;
use crate::ssh_util::{self, create_askpass_helper};
use std::fs;
use std::path::Path;
use std::process::Command;

const WINDOW_TITLE_PREFIX: &str = "Script Reloader - exec";

pub fn launch_exec_terminal(
    connection: &ConnectionDraft,
    container_target: &str,
    display_name: &str,
) -> Result<(), String> {
    if connection.host.is_empty()
        || connection.username.is_empty()
        || connection.password.is_empty()
    {
        return Err("Not connected. Connect before opening exec terminal.".into());
    }
    if container_target.trim().is_empty() {
        return Err("Container target is required.".into());
    }

    let remote = format!(
        "docker exec -it '{}' bash",
        escape_for_single_quoted_unix_shell(container_target)
    );
    let title = format!("{WINDOW_TITLE_PREFIX} {display_name}");

    let askpass = create_askpass_helper(&connection.password)?;
    let ssh_command = ssh_util::build_ssh_shell_command(connection, &remote, true);

    let result = try_windows_terminal(&title, &askpass, &ssh_command)
        .or_else(|_| try_cmd_start(&title, &askpass, &ssh_command));

    let _ = fs::remove_file(&askpass);
    result
}

fn try_windows_terminal(title: &str, askpass: &Path, ssh_command: &str) -> Result<(), String> {
    if ssh_util::which_on_path("wt.exe").is_none() {
        return Err("wt.exe not on PATH".into());
    }

    let inner = format!(
        "set SSH_ASKPASS={} && set DISPLAY=1 && set SSH_ASKPASS_REQUIRE=force && {}",
        askpass.display(),
        ssh_command
    );

    Command::new("wt.exe")
        .args(["--title", title, "cmd.exe", "/C", &inner])
        .spawn()
        .map_err(|e| format!("Failed to launch Windows Terminal: {e}"))?;

    Ok(())
}

fn try_cmd_start(title: &str, askpass: &Path, ssh_command: &str) -> Result<(), String> {
    let inner = format!(
        "set SSH_ASKPASS={} && set DISPLAY=1 && set SSH_ASKPASS_REQUIRE=force && {} && pause",
        askpass.display(),
        ssh_command
    );

    Command::new("cmd.exe")
        .args(["/C", "start", title, "cmd.exe", "/K", &inner])
        .spawn()
        .map_err(|e| format!("Failed to launch terminal: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh_util;

    #[test]
    fn remote_command_quotes_target() {
        let cmd = ssh_util::build_ssh_shell_command(
            &ConnectionDraft {
                host: "host".into(),
                port: 22,
                username: "user".into(),
                password: "pw".into(),
                command_timeout_seconds: 120,
            },
            "docker exec -it 'abc' bash",
            true,
        );
        assert!(cmd.contains("user@host"));
        assert!(cmd.contains("-t"));
    }
}
