# The Dark Forest Theory: put the project in a private GitHub repo.
# Copies the unzipped source to source\repos\DarkForestTheory, installs Git and
# the GitHub CLI if they're missing, makes the first commit and pushes it.
param(
    [string]$Source = "$env:USERPROFILE\Downloads\DarkForestTheory-source",
    [string]$Dest = "$env:USERPROFILE\source\repos\DarkForestTheory",
    [string]$RepoName = "dft"
)
# (Continue, not Stop: Windows PowerShell treats git's and gh's progress
# messages on stderr as errors. Every step checks its own result.)
$ErrorActionPreference = "Continue"

function Step($text) { Write-Host ""; Write-Host "== $text" -ForegroundColor Cyan }
function Fail($text) { Write-Host ""; Write-Host "STOPPED: $text" -ForegroundColor Red; exit 1 }
function Refresh-Path {
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [Environment]::GetEnvironmentVariable("Path", "User")
}
function Have($cmd) { [bool](Get-Command $cmd -ErrorAction SilentlyContinue) }
function Run($exe, [string[]]$argList) {
    & $exe @argList
    if ($LASTEXITCODE -ne 0) { Fail "$exe $($argList -join ' ') failed (exit $LASTEXITCODE)" }
}
function Write-Text($path, $text) {
    # UTF-8 without a byte-order mark, Unix line endings.
    [IO.File]::WriteAllText($path, ($text -replace "`r`n", "`n"), (New-Object Text.UTF8Encoding($false)))
}

# --- 1. Find the project -----------------------------------------------------
Step "Looking for the project in $Source"
if (-not (Test-Path $Source)) {
    if (Test-Path (Join-Path $Dest ".git")) { $Source = $Dest } else { Fail "no folder at $Source. Unzip DarkForestTheory-source.zip there first." }
}
$root = $Source
if (-not (Test-Path (Join-Path $root "Cargo.toml"))) {
    # Unzipping often adds one more folder level.
    $inner = Get-ChildItem $Source -Directory | Where-Object { Test-Path (Join-Path $_.FullName "Cargo.toml") } | Select-Object -First 1
    if (-not $inner) { Fail "can't find Cargo.toml in $Source or the folder inside it." }
    $root = $inner.FullName
}
Write-Host "Found it: $root"

# --- 2. Copy it to source\repos -----------------------------------------------
$resume = Test-Path (Join-Path $Dest ".git")
if ($resume) {
    Step "$Dest is already a git repo: picking up where the last run stopped"
} else {
    Step "Copying to $Dest"
    if ((Test-Path $Dest) -and (Get-ChildItem $Dest -Force | Select-Object -First 1)) {
        Fail "$Dest already exists and isn't empty. Move or rename it, then run this again."
    }
    New-Item -ItemType Directory -Force -Path $Dest | Out-Null
    robocopy $root $Dest /E /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { Fail "copying failed (robocopy exit $LASTEXITCODE)" }
    Write-Host "Copied $((Get-ChildItem $Dest -Recurse -File).Count) files."
}
Set-Location $Dest

# --- 3. Git and the GitHub CLI --------------------------------------------------
Step "Checking for Git and the GitHub CLI"
if (-not (Have "winget") -and (-not (Have "git") -or -not (Have "gh"))) {
    Fail "winget isn't available to install the tools. Install Git (git-scm.com) and GitHub CLI (cli.github.com), then run this again."
}
if (-not (Have "git")) {
    Write-Host "Installing Git..."
    winget install --id Git.Git -e --source winget --accept-package-agreements --accept-source-agreements
    Refresh-Path
    if (-not (Have "git")) { Fail "Git installed but isn't on the PATH yet. Close this window and run the script again." }
}
if (-not (Have "gh")) {
    Write-Host "Installing GitHub CLI..."
    winget install --id GitHub.cli -e --source winget --accept-package-agreements --accept-source-agreements
    Refresh-Path
    if (-not (Have "gh")) { Fail "GitHub CLI installed but isn't on the PATH yet. Close this window and run the script again." }
}
git --version
gh --version | Select-Object -First 1

# --- 4. Who you are, for commits --------------------------------------------------
Step "Commit identity"
$name = git config --global user.name
$email = git config --global user.email
if (-not $name) { $name = Read-Host "Your name for commits"; git config --global user.name "$name" }
if (-not $email) { $email = Read-Host "Your email for commits (the one on your GitHub account)"; git config --global user.email "$email" }
Write-Host "Committing as $name <$email>"

if (-not $resume) {
# --- 5. Ignore rules and line endings ----------------------------------------------
Step "Writing .gitignore and .gitattributes"
Write-Text (Join-Path $Dest ".gitignore") @"
target/
logs/
saves/
out/
*-log.txt
viewer/pkg/
viewer/dist/
"@
Write-Text (Join-Path $Dest ".gitattributes") @"
* text=auto eol=lf
*.bat text eol=crlf
*.ps1 text eol=crlf
*.png binary
"@

# --- 6. First commit ------------------------------------------------------------
Step "First commit"
Run git @("init", "-b", "main")
Run git @("add", ".")
Run git @("commit", "-q", "-m", "Initial import: The Dark Forest Theory")
git log --oneline -1
}

# --- 7. GitHub ------------------------------------------------------------------
Step "Signing in to GitHub"
gh auth status 2>$null
if ($LASTEXITCODE -ne 0) {
    Write-Host "A browser window will open: sign in and approve the GitHub CLI."
    gh auth login --hostname github.com --git-protocol https --web
    if ($LASTEXITCODE -ne 0) { Fail "GitHub sign-in didn't finish. Run this again: it picks up from here." }
}
# Let git push with the GitHub CLI's sign-in.
gh auth setup-git | Out-Null

git remote get-url origin 2>$null | Out-Null
if ($LASTEXITCODE -eq 0) {
    Step "Pushing to the repo made last time"
    Run git @("push", "-u", "origin", "main")
} else {
  $existing = gh repo view $RepoName --json url -q .url 2>$null
  if ($LASTEXITCODE -eq 0 -and $existing) {
    # The repo is already on GitHub: point at it and push.
    Step "Pushing to your existing repo $existing"
    Run git @("remote", "add", "origin", "$existing.git")
    Run git @("push", "-u", "origin", "main")
  } else {
    Step "Creating the private repo $RepoName and pushing"
    Run gh @("repo", "create", $RepoName, "--private", "--source", ".", "--remote", "origin", "--push")
  }
}

$url = gh repo view --json url -q .url
Write-Host ""
Write-Host "Done." -ForegroundColor Green
Write-Host "Project folder: $Dest"
Write-Host "GitHub repo:    $url"
Write-Host "Next: run install-rust.bat in the project folder if Rust isn't on this PC yet."
