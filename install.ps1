#Requires -Version 5
# Install script for homeos
# Usage: irm https://raw.githubusercontent.com/homeos-dev/homeos/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "homeos-dev/homeos"
$InstallDir = if ($env:HOMEOS_INSTALL_DIR) { $env:HOMEOS_INSTALL_DIR } else { "$env:USERPROFILE\.homeos\bin" }

function Test-AlreadyLatest {
    if ($env:HOMEOS_FORCE_INSTALL) { return $false }
    if (-not (Get-Command homeos -ErrorAction SilentlyContinue)) { return $false }
    $LocalVersionLine = & homeos --version 2>$null
    if (-not $LocalVersionLine) { return $false }
    $LocalVersion = ($LocalVersionLine -split '\s+')[-1]
    if (-not $LocalVersion) { return $false }
    try {
        $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -TimeoutSec 5 -ErrorAction Stop
    } catch {
        return $false
    }
    $LatestVersion = if ($Release.tag_name) { $Release.tag_name -replace '^v', '' } else { $null }
    if (-not $LatestVersion) { return $false }
    if ($LocalVersion -ne $LatestVersion) { return $false }
    Write-Host "homeos $LocalVersion is already the latest. Set HOMEOS_FORCE_INSTALL=1 to reinstall."
    return $true
}

if (-not (Test-AlreadyLatest)) {
    $Arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    $Target = switch ($Arch) {
        "AMD64" { "x86_64-pc-windows-msvc" }
        "ARM64" { "aarch64-pc-windows-msvc" }
        default { throw "Unsupported architecture: $Arch" }
    }

    $Url = "https://github.com/$Repo/releases/latest/download/homeos-$Target.zip"

    $TempDir = Join-Path ([System.IO.Path]::GetTempPath()) "homeos-install-$([guid]::NewGuid())"
    New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
    $TempZip = Join-Path $TempDir "homeos.zip"

    try {
        Write-Host "Downloading homeos for $Target..."
        Invoke-WebRequest -Uri $Url -OutFile $TempZip -UseBasicParsing

        Write-Host "Extracting..."
        Expand-Archive -Path $TempZip -DestinationPath $TempDir -Force

        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        }
        $TargetExe = Join-Path $InstallDir "homeos.exe"
        if (Test-Path $TargetExe) {
            $Suffix = [guid]::NewGuid().ToString("N").Substring(0, 8)
            $OldName = "homeos.exe.old-$Suffix"
            try {
                Rename-Item -Path $TargetExe -NewName $OldName -ErrorAction Stop
            } catch {
                throw "Failed to rename existing $TargetExe. Close any running 'homeos' processes and any terminals where homeos was recently invoked, then re-run the installer. Original error: $_"
            }
        }
        Move-Item -Path (Join-Path $TempDir "homeos.exe") -Destination $TargetExe -Force

        Get-ChildItem -Path $InstallDir -Filter "homeos.exe.old-*" -ErrorAction SilentlyContinue | ForEach-Object {
            try {
                Remove-Item $_.FullName -Force -ErrorAction Stop
            } catch {
                # Still locked by a running process; the next install will retry.
            }
        }

        Write-Host "Installed homeos to $TargetExe"

        $CompletionDir = Join-Path $env:USERPROFILE ".homeos"
        $CompletionFile = Join-Path $CompletionDir "completion.ps1"
        if (-not (Test-Path $CompletionDir)) {
            New-Item -ItemType Directory -Force -Path $CompletionDir | Out-Null
        }
        & (Join-Path $InstallDir "homeos.exe") completion powershell | Out-File -FilePath $CompletionFile -Encoding utf8
        Write-Host ""
        Write-Host "Installed PowerShell completion to $CompletionFile"
        Write-Host "Add the following line to your `$PROFILE to enable completion:"
        Write-Host ""
        Write-Host "    . `"$CompletionFile`""
        Write-Host ""

        $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $PathParts = if ($UserPath) { $UserPath -split ';' | Where-Object { $_ -ne '' } } else { @() }
        if ($PathParts -notcontains $InstallDir) {
            $NewPath = ((@($PathParts) + $InstallDir) -join ';')
            [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
            Write-Host ""
            Write-Host "Added $InstallDir to your user PATH."
            Write-Host "Open a new terminal to use 'homeos'."
        } else {
            Write-Host ""
            & (Join-Path $InstallDir "homeos.exe") --version
        }
    } finally {
        if (Test-Path $TempDir) { Remove-Item $TempDir -Recurse -Force }
    }
}
