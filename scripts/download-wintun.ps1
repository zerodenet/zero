# Download wintun.dll for Windows TUN support.
# Requires: Windows, Administrator (for TUN usage, not for download).
#
# Usage: powershell -File scripts\download-wintun.ps1

$ErrorActionPreference = "Stop"

$version = "0.14.1"
$url = "https://www.wintun.net/builds/wintun-$version.zip"
$zip = "$env:TEMP\wintun-$version.zip"
$extract = "$env:TEMP\wintun-$version"
$target = if ($args[0]) { $args[0] } else { "target\debug\" }

Write-Host "Downloading wintun $version..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $url -OutFile $zip

Write-Host "Extracting..." -ForegroundColor Cyan
Expand-Archive -Path $zip -DestinationPath $extract -Force

$arch = if ([Environment]::Is64BitOperatingSystem) { "amd64" } else { "x86" }
$dll = Join-Path $extract "wintun-$version" "wintun.dll"

if (Test-Path $dll) {
    Copy-Item $dll $target -Force
    Write-Host "wintun.dll copied to $target" -ForegroundColor Green
} else {
    Write-Error "wintun.dll not found in archive at $dll"
}

Remove-Item $zip -Force -ErrorAction SilentlyContinue
Remove-Item $extract -Recurse -Force -ErrorAction SilentlyContinue
