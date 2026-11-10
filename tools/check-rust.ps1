# Checks everything this project needs to build on this PC and writes rust-check.txt.
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
$log = Join-Path $root 'rust-check.txt'
"Rust check, $(Get-Date)" | Set-Content $log
function Say($t) { Write-Host $t; Add-Content $log $t }
function Run($label, $exe, $argList) {
  Say "--- $label"
  try {
    $out = & $exe @argList 2>&1 | Out-String
    Say ($out.TrimEnd())
    Say "(exit $LASTEXITCODE)"
  } catch { Say "NOT FOUND: $($_.Exception.Message)" }
}

Say "Windows: $([Environment]::OSVersion.VersionString)  PowerShell $($PSVersionTable.PSVersion)"
Say "USERPROFILE: $env:USERPROFILE"
Say "CARGO_HOME: $env:CARGO_HOME   RUSTUP_HOME: $env:RUSTUP_HOME"
$cb = Join-Path $env:USERPROFILE '.cargo\bin'
Say "Default .cargo\bin exists: $(Test-Path $cb)"
Say "On PATH: cargo=$((Get-Command cargo -ErrorAction SilentlyContinue).Source)  rustc=$((Get-Command rustc -ErrorAction SilentlyContinue).Source)  rustup=$((Get-Command rustup -ErrorAction SilentlyContinue).Source)"

Run 'rustc --version' 'rustc' @('--version')
Run 'cargo --version' 'cargo' @('--version')
Run 'rustup show' 'rustup' @('show')

Say '--- MSVC linker (Visual Studio C++ build tools)'
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
  $all = & $vswhere -all -products * -format text 2>&1 | Select-String 'displayName|installationVersion|installationPath' | Out-String
  Say ($all.TrimEnd())
  $vc = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  if ($vc) { Say "C++ x64 tools installed at: $vc" } else { Say 'C++ x64 build tools: NOT INSTALLED' }
} else { Say 'vswhere not found: no Visual Studio installer on this PC' }

Run 'python --version' 'python' @('--version')

Say '--- Test build (tiny program, checks the compiler and linker together)'
if (Get-Command cargo -ErrorAction SilentlyContinue) {
  $t = Join-Path $env:TEMP 'dft-rust-check'
  Remove-Item $t -Recurse -Force -ErrorAction SilentlyContinue
  & cargo new --quiet $t 2>&1 | Out-Null
  Push-Location $t
  $o = & cargo run --quiet 2>&1 | Out-String
  Say ($o.TrimEnd())
  if ($LASTEXITCODE -eq 0) { Say 'TEST BUILD: OK' } else { Say 'TEST BUILD: FAILED' }
  Pop-Location
} else { Say 'skipped: cargo not found' }

Say ''
Say "Saved to $log"
