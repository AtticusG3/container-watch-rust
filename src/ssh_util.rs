//! System OpenSSH client helpers (password via SSH_ASKPASS).

use crate::models::ConnectionDraft;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct SshCommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn find_ssh_exe() -> Result<PathBuf, String> {
    which_on_path("ssh.exe").ok_or_else(|| {
        "OpenSSH client (ssh.exe) not found on PATH. Install the Windows OpenSSH Client.".into()
    })
}

pub fn create_askpass_helper(password: &str) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!(
        "container-watch-askpass-{}.cmd",
        std::process::id()
    ));

    let escaped = password.replace('\'', "''");
    let mut file = fs::File::create(&path).map_err(|e| format!("askpass create failed: {e}"))?;
    writeln!(file, "@echo off").map_err(|e| format!("askpass write failed: {e}"))?;
    writeln!(
        file,
        "powershell -NoProfile -Command \"Write-Output '{}'\"",
        escaped
    )
    .map_err(|e| format!("askpass write failed: {e}"))?;
    Ok(path)
}

pub fn shell_escape_double_quotes(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

pub fn build_ssh_shell_command(connection: &ConnectionDraft, remote_command: &str, tty: bool) -> String {
    let tty_flag = if tty { "-t " } else { "" };
    format!(
        "ssh {tty_flag}-p {} -o StrictHostKeyChecking=accept-new -o PreferredAuthentications=password -o PubkeyAuthentication=no -o NumberOfPasswordPrompts=1 {}@{} {}",
        connection.port,
        connection.username,
        connection.host,
        shell_escape_double_quotes(remote_command)
    )
}

pub fn run_ssh_command(
    connection: &ConnectionDraft,
    remote_command: &str,
    timeout: Duration,
    allocate_tty: bool,
) -> Result<SshCommandResult, String> {
    let ssh = find_ssh_exe()?;
    let askpass = create_askpass_helper(&connection.password)?;

    let mut cmd = Command::new(&ssh);
    cmd.env("SSH_ASKPASS", &askpass);
    cmd.env("DISPLAY", "1");
    cmd.env("SSH_ASKPASS_REQUIRE", "force");

    cmd.args([
        "-p",
        &connection.port.to_string(),
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
        "PreferredAuthentications=password",
        "-o",
        "PubkeyAuthentication=no",
        "-o",
        "NumberOfPasswordPrompts=1",
    ]);
    if allocate_tty {
        cmd.arg("-t");
    }
    cmd.arg(format!("{}@{}", connection.username, connection.host));
    cmd.arg(remote_command);

    hide_console_window(&mut cmd);

    let result = run_with_timeout(cmd, timeout);
    let _ = fs::remove_file(&askpass);
    result
}

fn run_with_timeout(mut cmd: Command, timeout: Duration) -> Result<SshCommandResult, String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = cmd.output().map_err(|e| format!("SSH process failed: {e}"));
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => Ok(map_output(output)),
        Ok(Err(err)) => Err(err),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "SSH command timed out after {}s.",
            timeout.as_secs()
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("SSH worker thread panicked.".into())
        }
    }
}

fn map_output(output: Output) -> SshCommandResult {
    SshCommandResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn hide_console_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

pub fn which_on_path(exe: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(exe))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_ssh_shell_command_includes_user_host() {
        let cmd = build_ssh_shell_command(
            &ConnectionDraft {
                host: "host".into(),
                port: 2222,
                username: "user".into(),
                password: "pw".into(),
                command_timeout_seconds: 120,
            },
            "docker ps",
            false,
        );
        assert!(cmd.contains("user@host"));
        assert!(cmd.contains("-p 2222"));
    }
}
