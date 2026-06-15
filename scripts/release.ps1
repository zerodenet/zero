#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Bump the Zero project version, commit, tag, and push to all remotes.

.DESCRIPTION
    Updates the workspace.package.version field in the root Cargo.toml,
    commits the change, creates an annotated git tag (v<version>), and
    pushes the commit + tag to every configured remote.

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
    Write-Host "[DRY RUN] Would update Cargo.toml, commit, tag $tagName, push to: $($remotes -join ', ')" -ForegroundColor Green
    exit 0
}

# ---------- Confirm ----------
$confirm = Read-Host "Proceed with release v$Version? [y/N]"
if ($confirm -notmatch '^[yY]') {
    Write-Host "Aborted." -ForegroundColor Red
    exit 0
}

# ---------- Update Cargo.toml ----------
Write-Host "Updating version in $cargoTomlPath..." -ForegroundColor Cyan
$updated = $cargoContent -replace "(\[workspace\.package\][^\[]*?version\s*=\s*`")[^`"]+(`")", "`${1}$Version`${2}"
if ($updated -eq $cargoContent) {
    Write-Error "Failed to update version in Cargo.toml — pattern not matched."
    exit 1
}
Set-Content -Path $cargoTomlPath -Value $updated -NoNewline
Write-Host "  version = `"$Version`"" -ForegroundColor Green

# ---------- Commit ----------
Write-Host "Committing..." -ForegroundColor Cyan
git add Cargo.toml
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
