use crate::event_protocol::DEFAULT_CANVAS_API;

#[cfg(windows)]
fn windows_shell() -> String {
    if is_in_path("pwsh.exe") {
        "pwsh.exe".to_owned()
    } else {
        "powershell.exe".to_owned()
    }
}

#[cfg(windows)]
fn is_in_path(exe: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };

    std::env::split_paths(&path).any(|dir| dir.join(exe).is_file())
}

pub fn system_shell() -> String {
    #[cfg(windows)]
    {
        windows_shell()
    }

    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_owned())
    }
}

/// Embedded PowerShell init script template.
/// Placeholders: %NODE_ID%, %NODE_UID%, %CANVAS_API%, %CANVAS_ROOT%
/// See assets/init-canvas-terminal.ps1 for the readable version.
const WINDOWS_PTY_INIT_SCRIPT: &str =
    include_str!("../assets/init-canvas-terminal.ps1");

pub fn terminal_shell_args(node_id: usize, node_uid: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        let escaped_node_uid = node_uid.replace('\'', "''");
        let escaped_api = DEFAULT_CANVAS_API.replace('\'', "''");
        let escaped_cwd = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().replace('\'', "''"))
            .unwrap_or_default();
        let cmd = WINDOWS_PTY_INIT_SCRIPT
            .replace("%NODE_ID%", &node_id.to_string())
            .replace("%NODE_UID%", &escaped_node_uid)
            .replace("%CANVAS_API%", &escaped_api)
            .replace("%CANVAS_ROOT%", &escaped_cwd);
        vec![
            "-NoExit".to_owned(),
            "-Command".to_owned(),
            cmd,
        ]
    }

    #[cfg(not(windows))]
    {
        let escaped_node_uid = node_uid.replace('\\', "\\\\").replace('"', "\\\"");
        vec![
            "-lc".to_owned(),
            format!(
                "export CANVAS_NODE_ID={node_id} CANVAS_NODE_UID=\"{escaped_node_uid}\" CANVAS_API=\"{DEFAULT_CANVAS_API}\"; exec {}",
                system_shell()
            ),
        ]
    }
}
