//! Remote Docker operations over SSH via system `ssh.exe` (password auth, serialized execs).

use crate::models::{ConnectionDraft, ContainerInfo};
use crate::ssh_util::{self, SshCommandResult};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct DockerCommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl From<SshCommandResult> for DockerCommandResult {
    fn from(value: SshCommandResult) -> Self {
        Self {
            exit_code: value.exit_code,
            stdout: value.stdout,
            stderr: value.stderr,
        }
    }
}

#[derive(Clone)]
pub struct SshDockerService {
    inner: Arc<Mutex<SshDockerInner>>,
    gate: Arc<Mutex<()>>,
}

struct SshDockerInner {
    connection: Option<ConnectionDraft>,
}

impl Default for SshDockerService {
    fn default() -> Self {
        Self::new()
    }
}

impl SshDockerService {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SshDockerInner { connection: None })),
            gate: Arc::new(Mutex::new(())),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.connection.as_ref().map(|_| true))
            .unwrap_or(false)
    }

    pub fn connect(
        &self,
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        if host.trim().is_empty() || username.trim().is_empty() {
            return Err("Host and username are required.".into());
        }

        let draft = ConnectionDraft {
            host: host.to_string(),
            port,
            username: username.to_string(),
            password: password.to_string(),
            command_timeout_seconds: timeout.as_secs(),
        };

        let probe = ssh_util::run_ssh_command(&draft, "echo connected", timeout, false)?;
        if probe.exit_code != 0 {
            let result: DockerCommandResult = probe.into();
            return Err(build_docker_error("SSH connect", &result));
        }

        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "SSH service lock poisoned".to_string())?;
        guard.connection = Some(draft);
        Ok(())
    }

    pub fn disconnect(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.connection = None;
        }
    }

    pub fn list_containers(&self, timeout: Duration) -> Result<Vec<ContainerInfo>, String> {
        let result = self.run_command("docker ps -a --format '{{json .}}'", timeout)?;
        if result.exit_code != 0 {
            return Err(build_docker_error("docker ps", &result));
        }

        let mut list = Vec::new();
        let mut parse_failures = 0usize;
        for line in result
            .stdout
            .split('\n')
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            match serde_json::from_str::<ContainerInfo>(line) {
                Ok(row) => list.push(row),
                Err(_) => parse_failures += 1,
            }
        }

        let trimmed = result.stdout.trim();
        if list.is_empty() && !trimmed.is_empty() {
            let sample = trimmed.lines().next().unwrap_or("");
            let sample = truncate(sample, 240);
            let mut hint = String::from("docker ps returned output but no rows could be parsed.");
            if parse_failures > 0 {
                hint.push_str(&format!(" JSON parse failures: {parse_failures}."));
            }
            hint.push_str(&format!(" First line: {sample}"));
            if !result.stderr.trim().is_empty() {
                hint.push_str(&format!(" stderr: {}", result.stderr.trim()));
            }
            return Err(hint);
        }

        Ok(list)
    }

    pub fn restart_containers(
        &self,
        targets: &[String],
        timeout: Duration,
    ) -> Result<(), String> {
        let args = build_quoted_container_args(targets)?;
        let cmd = format!("docker restart --time 10 {args}");
        let result = self.run_command(&cmd, timeout)?;
        if result.exit_code != 0 {
            return Err(build_docker_error("docker restart", &result));
        }
        Ok(())
    }

    pub fn restart_all_running(&self, timeout: Duration) -> Result<(), String> {
        let list_result = self.run_command("docker ps -q", timeout)?;
        if list_result.exit_code != 0 {
            return Err(build_docker_error("docker ps -q", &list_result));
        }

        let ids: Vec<String> = list_result
            .stdout
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();

        if ids.is_empty() {
            return Ok(());
        }

        self.restart_containers(&ids, timeout)
    }

    pub fn stop_containers(&self, targets: &[String], timeout: Duration) -> Result<(), String> {
        let args = build_quoted_container_args(targets)?;
        let cmd = format!("docker stop --time 10 {args}");
        let result = self.run_command(&cmd, timeout)?;
        if result.exit_code != 0 {
            return Err(build_docker_error("docker stop", &result));
        }
        Ok(())
    }

    pub fn start_containers(&self, targets: &[String], timeout: Duration) -> Result<(), String> {
        let args = build_quoted_container_args(targets)?;
        let cmd = format!("docker start {args}");
        let result = self.run_command(&cmd, timeout)?;
        if result.exit_code != 0 {
            return Err(build_docker_error("docker start", &result));
        }
        Ok(())
    }

    fn run_command(&self, remote_command: &str, timeout: Duration) -> Result<DockerCommandResult, String> {
        let inner = Arc::clone(&self.inner);
        let gate = Arc::clone(&self.gate);
        let cmd = remote_command.to_string();

        thread::spawn(move || run_command_locked(&inner, &gate, &cmd, timeout))
            .join()
            .map_err(|_| "SSH worker thread panicked".to_string())?
    }
}

fn run_command_locked(
    inner: &Arc<Mutex<SshDockerInner>>,
    gate: &Arc<Mutex<()>>,
    remote_command: &str,
    timeout: Duration,
) -> Result<DockerCommandResult, String> {
    let _gate = gate
        .lock()
        .map_err(|_| "SSH service lock poisoned".to_string())?;

    let connection = {
        let guard = inner
            .lock()
            .map_err(|_| "SSH service lock poisoned".to_string())?;
        guard
            .connection
            .as_ref()
            .ok_or_else(|| "Not connected. Connect before running Docker commands.".to_string())?
            .clone()
    };

    ssh_util::run_ssh_command(&connection, remote_command, timeout, false).map(Into::into)
}

/// Space-separated single-quoted container ids/names for POSIX shell.
pub fn build_quoted_container_args(container_ids_or_names: &[String]) -> Result<String, String> {
    if container_ids_or_names.is_empty() {
        return Err("At least one container id or name is required.".into());
    }

    let parts: Result<Vec<_>, _> = container_ids_or_names
        .iter()
        .map(|raw| {
            if raw.trim().is_empty() {
                Err("Container id or name cannot be empty.".to_string())
            } else {
                Ok(format!("'{}'", escape_for_single_quoted_unix_shell(raw)))
            }
        })
        .collect();

    Ok(parts?.join(" "))
}

pub fn escape_for_single_quoted_unix_shell(value: &str) -> String {
    value.replace('\'', "'\\''")
}

pub fn build_docker_error(label: &str, result: &DockerCommandResult) -> String {
    let mut msg = format!("{} failed (exit {}).", label, result.exit_code);
    if !result.stderr.trim().is_empty() {
        msg.push(' ');
        msg.push_str(result.stderr.trim());
    } else if !result.stdout.trim().is_empty() {
        msg.push(' ');
        msg.push_str(result.stdout.trim());
    }
    msg
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

pub fn build_action_confirm_message(verb: &str, names: &[(&str, &str)]) -> String {
    if names.is_empty() {
        return String::new();
    }
    if names.len() == 1 {
        let (display, target) = names[0];
        return format!("{verb} container \"{display}\" ({target})?");
    }

    const MAX_LIST: usize = 12;
    let mut body: String = names
        .iter()
        .take(MAX_LIST)
        .map(|(display, _)| format!("- {display}"))
        .collect::<Vec<_>>()
        .join("\n");

    if names.len() > MAX_LIST {
        body.push_str(&format!("\n... and {} more.", names.len() - MAX_LIST));
    }

    format!("{verb} {} containers?\n\n{body}", names.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quoting_escapes_single_quotes() {
        assert_eq!(escape_for_single_quoted_unix_shell("a'b"), "a'\\''b");
    }

    #[test]
    fn build_quoted_args_joins() {
        let args = build_quoted_container_args(&["abc".into(), "my-name".into()]).unwrap();
        assert_eq!(args, "'abc' 'my-name'");
    }

    #[test]
    fn build_quoted_args_rejects_empty() {
        assert!(build_quoted_container_args(&[]).is_err());
        assert!(build_quoted_container_args(&["".into()]).is_err());
    }

    #[test]
    fn confirm_message_single() {
        let msg = build_action_confirm_message("Stop", &[("web", "abc123")]);
        assert!(msg.contains("Stop container \"web\" (abc123)?"));
    }

    #[test]
    fn confirm_message_multi() {
        let names: Vec<(&str, &str)> = (0..15).map(|_| ("n", "id")).collect();
        let msg = build_action_confirm_message("Restart", &names);
        assert!(msg.contains("Restart 15 containers?"));
        assert!(msg.contains("... and 3 more."));
    }
}
