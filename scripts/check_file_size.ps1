#!/usr/bin/env pwsh
# CI gate: fail if any .rs source file exceeds 300 lines.
# This enforces the project's own "Split files around 300 lines" rule.
#
# Usage: pwsh scripts/check_file_size.ps1
# Exit code 0 = all files within limit; 1 = violations found.

$ErrorActionPreference = "Stop"
$Limit = 300
$Violations = @()

$SourceFiles = Get-ChildItem -Path crates, protocols, src -Recurse -Filter *.rs -ErrorAction SilentlyContinue

foreach ($File in $SourceFiles) {
    $LineCount = (Get-Content $File.FullName | Measure-Object -Line).Lines
    if ($LineCount -gt $Limit) {
        $RelativePath = $File.FullName.Replace((Get-Location).Path + '\', '')
        $Violations += [PSCustomObject]@{
            Lines = $LineCount
            File = $RelativePath
        }
    }
}

if ($Violations.Count -gt 0) {
    Write-Host "FAIL: $($Violations.Count) file(s) exceed the $Limit-line limit:" -ForegroundColor Red
    $Violations | Sort-Object Lines -Descending | ForEach-Object {
        Write-Host ("  {0,5}  {1}" -f $_.Lines, $_.File) -ForegroundColor Yellow
    }
    Write-Host ""
    Write-Host "Split these files per AGENTS.md: 'Split files around 300 lines'." -ForegroundColor Red
    exit 1
}

Write-Host "OK: all .rs files within $Limit-line limit." -ForegroundColor Green
exit 0
