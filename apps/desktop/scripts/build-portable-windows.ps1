$ErrorActionPreference = 'Stop'
$desktop = Split-Path -Parent $PSScriptRoot
$repo = Split-Path -Parent (Split-Path -Parent $desktop)
$target = 'x86_64-pc-windows-msvc'
$version = (Get-Content -Raw (Join-Path $desktop 'package.json') | ConvertFrom-Json).version
$gitRoot = Split-Path -Parent (Split-Path -Parent (Get-Command git.exe).Source)
$bash = Join-Path $gitRoot 'bin\bash.exe'

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
