use crate::model::DecisionButton;
use crate::shell::system_shell;
use serde::Deserialize;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize)]
pub(in crate::app) struct GraphGetPayload {
    #[serde(default)]
    pub since_version: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(in crate::app) struct NodeCreatePayload {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub text_body: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub startup_script: Option<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub image_path: Option<String>,
    #[serde(default)]
    pub buttons: Option<Vec<DecisionButton>>,
    #[serde(default)]
    pub pending_message: Option<String>,
    #[serde(default)]
    pub pending_messages: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(in crate::app) struct NodeMovePayload {
    pub id: usize,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Deserialize)]
pub(in crate::app) struct NodeDeletePayload {
    pub id: usize,
}

#[derive(Debug, Deserialize)]
pub(in crate::app) struct NodeUpdatePayload {
    pub id: usize,
    #[serde(default)]
    pub text_body: Option<String>,
    #[serde(default)]
    pub auto_size: Option<bool>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub startup_script: Option<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub buttons: Option<Vec<DecisionButton>>,
    #[serde(default)]
    pub pending_message: Option<String>,
    #[serde(default)]
    pub pending_messages: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(in crate::app) struct EdgePayload {
    pub from: usize,
    pub to: usize,
    #[serde(default)]
    pub route_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(in crate::app) struct EdgeReconnectPayload {
    pub from: usize,
    pub to: usize,
    pub new_from: usize,
    pub new_to: usize,
    #[serde(default)]
    pub new_route_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(in crate::app) struct InjectTextPayload {
    pub node_id: usize,
    pub mode: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub(in crate::app) struct InjectTerminalPayload {
    pub node_id: usize,
    pub command: String,
    #[serde(default)]
    pub wait: Option<bool>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(in crate::app) struct TerminalRestartPayload {
    pub node_id: usize,
}

pub(in crate::app) struct ShellCommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

pub(in crate::app) fn run_shell_command(
    command: &str,
    timeout_ms: u64,
) -> Result<ShellCommandOutput, String> {
    let shell = system_shell();

    let mut child = if cfg!(windows) {
        Command::new(shell)
            .arg("-NoProfile")
            .arg("-Command")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| format!("failed to spawn shell: {err}"))?
    } else {
        Command::new(shell)
            .arg("-lc")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| format!("failed to spawn shell: {err}"))?
    };

    let started = Instant::now();
    loop {
        if let Some(_status) = child
            .try_wait()
            .map_err(|err| format!("wait failed: {err}"))?
        {
            let output = child
                .wait_with_output()
                .map_err(|err| format!("read output failed: {err}"))?;
            return Ok(ShellCommandOutput {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
                timed_out: false,
            });
        }

        if started.elapsed().as_millis() >= timeout_ms as u128 {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .map_err(|err| format!("read timeout output failed: {err}"))?;
            return Ok(ShellCommandOutput {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
                timed_out: true,
            });
        }

        std::thread::sleep(Duration::from_millis(20));
    }
}
