$ErrorActionPreference = 'Stop'
$desktop = Split-Path -Parent $PSScriptRoot
$repo = Split-Path -Parent (Split-Path -Parent $desktop)
$target = 'x86_64-pc-windows-msvc'
$version = (Get-Content -Raw (Join-Path $desktop 'package.json') | ConvertFrom-Json).version
function Resolve-GitBash {
    $candidates = [System.Collections.Generic.List[string]]::new()
    $gitCmd = Get-Command git.exe -ErrorAction SilentlyContinue
    if ($gitCmd -and $gitCmd.Source) {
        $dir = Split-Path -Parent $gitCmd.Source
        for ($i = 0; $i -lt 4 -and $dir; $i++) {
            $candidates.Add((Join-Path $dir 'bin\bash.exe'))
            $candidates.Add((Join-Path $dir 'usr\bin\bash.exe'))
            $dir = Split-Path -Parent $dir
        }
    }
    foreach ($root in @($env:ProgramFiles, ${env:ProgramFiles(x86)}, (Join-Path $env:LOCALAPPDATA 'Programs'))) {
        if ($root) {
            $candidates.Add((Join-Path $root 'Git\bin\bash.exe'))
            $candidates.Add((Join-Path $root 'Git\usr\bin\bash.exe'))
        }
    }
    foreach ($cmd in (Get-Command bash.exe -All -ErrorAction SilentlyContinue)) {
        if ($cmd.Source -and $cmd.Source -notmatch '\\(System32|SysWOW64|WindowsApps)\\bash\.exe$') {
            $candidates.Add($cmd.Source)
        }
    }
    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            return $candidate
        }
    }
    throw 'Could not locate Git Bash (bash.exe). Install Git for Windows or add Git\bin to PATH.'
}

$bash = Resolve-GitBash

Push-Location (Join-Path $desktop 'src-tauri')
try {
    $metadata = cargo metadata --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Could not locate the Cargo target directory.' }
    $targetDir = ($metadata | ConvertFrom-Json).target_directory
} finally {
    Pop-Location
}

$stagingRoot = Join-Path $targetDir "portable-stage-$([guid]::NewGuid())"
$stage = Join-Path $stagingRoot 'Munia'
$archiveDir = Join-Path $targetDir 'portable'
$archive = Join-Path $archiveDir "Munia-${version}-windows-x64.zip"
try {
    New-Item -ItemType Directory -Path $stage, $archiveDir -Force | Out-Null
    $fetch = Join-Path $PSScriptRoot 'fetch-llama-server-windows.sh'
    & $bash ($fetch.Replace('\', '/')) ($stage.Replace('\', '/'))
    if ($LASTEXITCODE -ne 0) { throw 'Could not stage the verified llama-server runtime.' }

    Push-Location $desktop
    try {
        & npm.cmd run tauri -- build --no-bundle --target $target
        if ($LASTEXITCODE -ne 0) { throw 'Native Tauri build failed.' }
    } finally {
        Pop-Location
    }

    Copy-Item -LiteralPath (Join-Path $targetDir "$target\release\munia.exe") -Destination $stage
    Copy-Item -LiteralPath (Join-Path $repo 'LICENSE'), (Join-Path $repo 'THIRD_PARTY_NOTICES.md') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $desktop 'src\brand\fonts\OFL.txt') -Destination (Join-Path $stage 'LICENSE-Chiron-GoRound.txt')
    Compress-Archive -LiteralPath $stage -DestinationPath $archive -Force
    Write-Output "Portable archive: $archive"
} finally {
    Remove-Item -LiteralPath $stagingRoot -Recurse -Force -ErrorAction SilentlyContinue
}
