#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Seal the tested Zero version, commit, tag, and push to all remotes.

.DESCRIPTION
    Moves the compatibility ledger's Unreleased entry to the requested
    version, updates workspace.package.version, commits both files, creates an
    annotated git tag (v<version>), and pushes the commit + tag to every
    configured remote.

.PARAMETER Version
    The new version string (e.g. "0.0.14" or "0.0.14-beta"). Required.

.PARAMETER DryRun
    If set, prints what would be done without making changes.

.PARAMETER NoPush
    If set, skips the final push step.

.PARAMETER Message
    Custom commit/tag message. Defaults to "release: v<version>".

.EXAMPLE
    ./scripts/release.ps1 -Version "0.0.14"
    ./scripts/release.ps1 -Version "0.0.14-beta" -DryRun
#>

param(
    [Parameter(Mandatory = $true, Position = 0, HelpMessage = "New version string (e.g. 0.0.14)")]
    [string]$Version,

    [Parameter(Mandatory = $false)]
    [switch]$DryRun,

    [Parameter(Mandatory = $false)]
    [switch]$NoPush,

    [Parameter(Mandatory = $false)]
    [string]$Message
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot\..

function Invoke-VersionContract {
    param([string[]]$ContractArgs)

    if (Get-Command py -ErrorAction SilentlyContinue) {
        & py -3 scripts/version_contract.py @ContractArgs
    }
    elseif (Get-Command python3 -ErrorAction SilentlyContinue) {
        & python3 scripts/version_contract.py @ContractArgs
    }
    elseif (Get-Command python -ErrorAction SilentlyContinue) {
        & python scripts/version_contract.py @ContractArgs
    }
    else {
        Write-Error "Python 3 is required to manage the release version contract."
    }

    if ($LASTEXITCODE -ne 0) {
        throw "Version contract command failed: $($ContractArgs -join ' ')"
    }
}

# ---------- Validate ----------
if ($Version -notmatch '^\d+\.\d+\.\d+(-[\w.]+)?$') {
    Write-Error "Invalid version format. Expected X.Y.Z or X.Y.Z-suffix. Got: '$Version'"
    exit 1
}

$tagName = "v$Version"
$Message = if ($Message) { $Message } else { "release: v$Version" }

# ---------- Check prerequisites ----------
$cargoTomlPath = "Cargo.toml"
if (-not (Test-Path $cargoTomlPath)) {
    Write-Error "Cargo.toml not found — run from the repo root."
    exit 1
}

Invoke-VersionContract @("check")

# Check clean working tree
$gitStatus = git status --porcelain
if ($gitStatus) {
    Write-Error "Working tree is not clean. Commit or stash changes before releasing."
    exit 1
}

# Check on a reasonable branch (warn on main/master, but allow)
$currentBranch = git rev-parse --abbrev-ref HEAD
Write-Host "Current branch: $currentBranch" -ForegroundColor Cyan

# ---------- Gather remotes ----------
$remotes = (git remote) | ForEach-Object { $_.Trim() } | Where-Object { $_ }
if (-not $remotes) {
    Write-Error "No git remotes configured."
    exit 1
}
Write-Host "Remotes: $($remotes -join ', ')" -ForegroundColor Cyan

# ---------- Read current version ----------
$cargoContent = Get-Content -Raw $cargoTomlPath
$currentPattern = 'version\s*=\s*"([^"]+)"'
$workspaceSection = [regex]::Match($cargoContent, '\[workspace\.package\][^\[]*').Value
$workspaceVersionMatch = [regex]::Match($workspaceSection, $currentPattern)
$currentVersion = if ($workspaceVersionMatch.Success) { $workspaceVersionMatch.Groups[1].Value } else { "unknown" }

Write-Host "Current version: $currentVersion -> New version: $Version" -ForegroundColor Yellow
Write-Host "Tag: $tagName" -ForegroundColor Yellow

if ($DryRun) {
    Invoke-VersionContract @("prepare-release", $Version, "--dry-run")
    Write-Host "[DRY RUN] Would update Cargo.toml and breaking-changes.md, commit, tag $tagName, push to: $($remotes -join ', ')" -ForegroundColor Green
    exit 0
}

# ---------- Confirm ----------
$confirm = Read-Host "Proceed with release v$Version? [y/N]"
if ($confirm -notmatch '^[yY]') {
    Write-Host "Aborted." -ForegroundColor Red
    exit 0
}

# ---------- Seal version contract ----------
Write-Host "Sealing Cargo and compatibility docs for $Version..." -ForegroundColor Cyan
Invoke-VersionContract @("prepare-release", $Version)
Invoke-VersionContract @("check-release", $Version)

# ---------- Commit ----------
Write-Host "Committing..." -ForegroundColor Cyan
git add Cargo.toml docs/control-plane-api/breaking-changes.md
git commit -m $Message
Write-Host "  commit: $Message" -ForegroundColor Green

# ---------- Tag ----------
Write-Host "Creating tag $tagName..." -ForegroundColor Cyan
git tag -a $tagName -m $Message
Write-Host "  tag: $tagName" -ForegroundColor Green

# ---------- Push ----------
if (-not $NoPush) {
    foreach ($remote in $remotes) {
        Write-Host "Pushing to $remote..." -ForegroundColor Cyan
        git push $remote $currentBranch
        git push $remote $tagName
        $msg = "  ${remote}: pushed ${currentBranch} + ${tagName}"
        Write-Host $msg -ForegroundColor Green
    }
    Write-Host "Done. Version $Version released and pushed." -ForegroundColor Green
}
else {
    Write-Host "Skipped push (--NoPush). Commit and tag are local only." -ForegroundColor Yellow
}
