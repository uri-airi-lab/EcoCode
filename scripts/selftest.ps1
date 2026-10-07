<#
.SYNOPSIS
    EcoCode self-test for Windows: builds, runs every test, measures a real
    workload with this machine's sensors, and writes selftest-report.txt.

.DESCRIPTION
    Run from a PowerShell prompt at the repository root:

        powershell -ExecutionPolicy Bypass -File scripts\selftest.ps1

    Needs Rust (rustup) and Python 3; pytest is optional (pip install pytest).
    Send selftest-report.txt back when something fails.
#>

$ErrorActionPreference = "Continue"
# ecocode prints UTF-8; read native command output as UTF-8 (Windows PowerShell
# defaults to the OEM code page).
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
$Report = Join-Path $Root "selftest-report.txt"
$Work = Join-Path ([System.IO.Path]::GetTempPath()) ("ecocode-selftest-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $Work | Out-Null
$OnWindows = ($env:OS -eq "Windows_NT")
$ExeName = if ($OnWindows) { "ecocode.exe" } else { "ecocode" }
$Bin = Join-Path $Root (Join-Path "target\release" $ExeName)
if (-not $OnWindows) { $Bin = Join-Path $Root "target/release/$ExeName" }
$Checks = Join-Path $Root "scripts\selftest_checks.py"
if (-not $OnWindows) { $Checks = Join-Path $Root "scripts/selftest_checks.py" }

$Py = $null
foreach ($candidate in @("python", "python3", "py")) {
    if (Get-Command $candidate -ErrorAction SilentlyContinue) { $Py = $candidate; break }
}

Set-Content -Path $Report -Value "" -Encoding UTF8
$Results = New-Object System.Collections.ArrayList

function Log([string]$Text) {
    Write-Host $Text
    Add-Content -Path $Report -Value $Text -Encoding UTF8
}

# Runs one step. The script block must leave $LASTEXITCODE non-zero (or throw) on failure.
function Step([string]$Name, [scriptblock]$Block) {
    Log ""
    Log "=== $Name"
    $global:LASTEXITCODE = 0
    $failed = $false
    try {
        $output = & $Block 2>&1 | ForEach-Object { "$_" }
        if ($LASTEXITCODE -ne 0) { $failed = $true }
    } catch {
        $output = @("$_")
        $failed = $true
    }
    $lines = @($output | Where-Object { $_ -notmatch "^test result: ok\. 0 passed" })
    $keep = if ($failed) { 60 } else { 15 }
    $lines | Select-Object -Last $keep | ForEach-Object { Log "$_" }
    # cargo test: one total line instead of one line per test binary.
    $totals = @(0, 0, 0)
    foreach ($line in $output) {
        if ($line -match "^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored") {
            $totals[0] += [int]$Matches[1]; $totals[1] += [int]$Matches[2]; $totals[2] += [int]$Matches[3]
        }
    }
    if (($totals[0] + $totals[1] + $totals[2]) -gt 0) {
        Log ("  total: {0} passed, {1} failed, {2} ignored" -f $totals[0], $totals[1], $totals[2])
    }
    if ($failed) { [void]$Results.Add("FAIL  $Name") } else { [void]$Results.Add("PASS  $Name") }
}

function Skip([string]$Name, [string]$Why) {
    Log ""
    Log "=== $Name"
    Log "  skipped: $Why"
    [void]$Results.Add("SKIP  $Name ($Why)")
}

function Summary {
    Log ""
    Log "=== summary"
    foreach ($r in $Results) { Log $r }
    Log ""
    Log "Report written to $Report"
    Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
}

Log ("EcoCode self-test - " + (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ"))
Log ([System.Environment]::OSVersion.VersionString + " / " + $env:PROCESSOR_ARCHITECTURE)
Log ((& cargo --version 2>&1 | Out-String).Trim() + " / " + (& rustc --version 2>&1 | Out-String).Trim())
if ($Py) { Log ("Python: " + (& $Py --version 2>&1 | Out-String).Trim()) } else { Log "Python: not found" }

Step "build (cargo build --release)" { cargo build --release --locked }
if (-not (Test-Path $Bin)) {
    Log "Build failed; stopping."
    Summary
    exit 1
}

Step "ecocode doctor" { & $Bin doctor }
$DoctorJson = Join-Path $Work "doctor.json"
& $Bin doctor --json | Set-Content -Path $DoctorJson -Encoding UTF8
$HasCpuSensor = @((Get-Content $DoctorJson -Raw | ConvertFrom-Json).channels | Where-Object { $_.domain -eq "cpu" }).Count -gt 0

Step "Rust tests (cargo test --workspace)" { cargo test --workspace --locked }
if ($HasCpuSensor) {
    Step "hardware tests (cargo test -- --ignored)" { cargo test --workspace --locked -- --ignored }
} else {
    Skip "hardware tests" "no CPU energy sensor (see ecocode doctor)"
}

if (-not $Py) {
    Skip "Python SDK tests" "Python not found"
    Skip "measured run" "Python not found"
} else {
    & $Py -c "import pytest" 2>$null
    if ($LASTEXITCODE -eq 0) {
        Step "Python SDK tests (pytest)" {
            $env:ECOCODE_BIN = $Bin
            & $Py -m pytest -q -p no:cacheprovider (Join-Path $Root "python/tests/unit") (Join-Path $Root "python/tests/e2e")
        }
    } else {
        Skip "Python SDK tests" "pytest not installed (pip install pytest)"
    }

    $RunJson = Join-Path $Work "run.json"
    Step "measured run: 10 s CPU burn under ecocode run" {
        & $Bin run -o json -f $RunJson -i 1 -- $Py $Checks burn 10
        if ($LASTEXITCODE -eq 0) { & $Py $Checks check-run $DoctorJson $RunJson }
    }
    $SdkJson = Join-Path $Work "sdk.json"
    Step "SDK standalone run (real sensors)" {
        $env:PATH = (Split-Path -Parent $Bin) + [System.IO.Path]::PathSeparator + $env:PATH
        & $Py $Checks sdk-standalone $DoctorJson $SdkJson
    }
    $AttachedJson = Join-Path $Work "attached.json"
    Step "SDK attached run (ecocode run -- python)" {
        & $Bin run -o json -f $AttachedJson -i 0.5 -- $Py $Checks sdk-attached-child
        if ($LASTEXITCODE -eq 0) { & $Py $Checks check-attached $AttachedJson }
    }
}

Summary
if ($Results | Where-Object { $_ -like "FAIL*" }) { exit 1 }
exit 0
