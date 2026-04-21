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

pub fn terminal_shell_args(node_id: usize, identity: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        let escaped_identity = identity.replace('\'', "''");
        let escaped_api = DEFAULT_CANVAS_API.replace('\'', "''");
        let escaped_cwd = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().replace('\'', "''"))
            .unwrap_or_default();
        vec![
            "-NoExit".to_owned(),
            "-Command".to_owned(),
            format!(
                "$env:CANVAS_NODE_ID='{node_id}'; $env:CANVAS_IDENTITY='{escaped_identity}'; $env:CANVAS_API='{escaped_api}'; $canvasRoot='{escaped_cwd}'; $machinePath=[Environment]::GetEnvironmentVariable('Path','Machine'); $userPath=[Environment]::GetEnvironmentVariable('Path','User'); $pathParts=@(); foreach ($rawPath in @($machinePath, $userPath, $env:Path)) {{ if ($rawPath) {{ foreach ($part in ($rawPath -split ';')) {{ $expanded=[Environment]::ExpandEnvironmentVariables($part).Trim(); if ($expanded) {{ $pathParts += $expanded }} }} }} }}; if ($env:USERPROFILE) {{ $pathParts += (Join-Path $env:USERPROFILE 'scoop\\shims') }}; $pathParts += 'C:\\Program Files\\Git\\cmd'; $pathParts += 'C:\\Program Files\\Git\\bin'; if ($canvasRoot) {{ $pathParts += $canvasRoot }}; $env:Path=(($pathParts | Where-Object {{ $_ -and $_.Trim() -ne '' }} | Select-Object -Unique) -join ';'); $env:PATH=$env:Path; $gitCandidates=@('C:\\Program Files\\Git\\cmd\\git.exe',(Join-Path $env:USERPROFILE 'scoop\\shims\\git.exe')); $gitExe=$gitCandidates | Where-Object {{ Test-Path $_ }} | Select-Object -First 1; if (-not (Get-Command git -ErrorAction SilentlyContinue) -and $gitExe) {{ function global:git {{ & $gitExe @args }} }}; $canvasCandidates=@((Join-Path $canvasRoot 'target\\debug\\canvas.exe'), (Join-Path $canvasRoot 'target\\release\\canvas.exe'), (Join-Path $canvasRoot 'dist\\canvas_skills\\canvas-agent-events\\bin\\canvas.exe'), (Join-Path $canvasRoot 'bin\\canvas.exe')); $canvasExe=$canvasCandidates | Where-Object {{ Test-Path $_ }} | Select-Object -First 1; if ($canvasExe) {{ function global:canvas {{ & $canvasExe @args }} }}; $starshipConfigCandidates=@((Join-Path $canvasRoot 'assets\\starship.toml'), (Join-Path $canvasRoot 'dist\\app\\starship.toml'), (Join-Path $canvasRoot 'starship.toml')); $starshipConfig=$starshipConfigCandidates | Where-Object {{ Test-Path $_ }} | Select-Object -First 1; if ($starshipConfig) {{ $env:STARSHIP_CONFIG=$starshipConfig; if (Get-Command starship -ErrorAction SilentlyContinue) {{ Invoke-Expression (& starship init powershell) }} }}"
            ),
        ]
    }

    #[cfg(not(windows))]
    {
        let escaped_identity = identity.replace('\\', "\\\\").replace('"', "\\\"");
        vec![
            "-lc".to_owned(),
            format!(
                "export CANVAS_NODE_ID={node_id} CANVAS_IDENTITY=\"{escaped_identity}\" CANVAS_API=\"{DEFAULT_CANVAS_API}\"; exec {}",
                system_shell()
            ),
        ]
    }
}
