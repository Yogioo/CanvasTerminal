# CanvasTerminal terminal init script
# Placeholders (replaced at compile-time by Rust): %NODE_ID%, %NODE_UID%, %CANVAS_API%, %CANVAS_ROOT%

$env:CANVAS_NODE_ID='%NODE_ID%'
$env:CANVAS_NODE_UID='%NODE_UID%'
$env:CANVAS_API='%CANVAS_API%'

$canvasRoot='%CANVAS_ROOT%'

# Build comprehensive PATH from machine + user + current
$machinePath=[Environment]::GetEnvironmentVariable('Path','Machine')
$userPath=[Environment]::GetEnvironmentVariable('Path','User')
$pathParts=@()
foreach ($rawPath in @($machinePath, $userPath, $env:Path)) {
    if ($rawPath) {
        foreach ($part in ($rawPath -split ';')) {
            $expanded=[Environment]::ExpandEnvironmentVariables($part).Trim()
            if ($expanded) { $pathParts += $expanded }
        }
    }
}
if ($env:USERPROFILE) { $pathParts += (Join-Path $env:USERPROFILE 'scoop\shims') }
$pathParts += 'C:\Program Files\Git\cmd'
$pathParts += 'C:\Program Files\Git\bin'
if ($canvasRoot) { $pathParts += $canvasRoot }
$env:Path=(($pathParts | Where-Object { $_ -and $_.Trim() -ne '' } | Select-Object -Unique) -join ';')
$env:PATH=$env:Path

# Define global git function if git.exe not in PATH
$gitCandidates=@(
    'C:\Program Files\Git\cmd\git.exe',
    (Join-Path $env:USERPROFILE 'scoop\shims\git.exe')
)
$gitExe=$gitCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not (Get-Command git -ErrorAction SilentlyContinue) -and $gitExe) {
    function global:git { & $gitExe @args }
}

# Define global canvas function by locating canvas.exe
$exeDir=$null
try {
    $procPath=(Get-Process -Id $PID).Path
    if ($procPath) { $exeDir=Split-Path -Parent $procPath }
} catch {}
$canvasCandidates=@()
if ($exeDir) { $canvasCandidates += (Join-Path $exeDir 'canvas.exe') }
if ($canvasRoot) {
    $canvasCandidates += (Join-Path $canvasRoot 'canvas.exe')
    $canvasCandidates += (Join-Path $canvasRoot 'dist\canvas.exe')
}
$canvasExe=$canvasCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($canvasExe) { function global:canvas { & $canvasExe @args } }

# Configure starship prompt
$starshipConfigCandidates=@(
    (Join-Path $canvasRoot 'assets\starship.toml'),
    (Join-Path $canvasRoot 'dist\starship.toml'),
    (Join-Path $canvasRoot 'starship.toml')
)
$starshipConfig=$starshipConfigCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($starshipConfig) {
    $env:STARSHIP_CONFIG=$starshipConfig
    if (Get-Command starship -ErrorAction SilentlyContinue) {
        Invoke-Expression (& starship init powershell)
    }
}
