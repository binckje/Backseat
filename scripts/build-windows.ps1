# Build interactif de Backseat sous Windows : clone/pull GitHub -> dépendances -> tauri build.
# Lancer : powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
$ErrorActionPreference = "Stop"
$RepoUrl = "https://github.com/binckje/Backseat.git"

function Ask($q, $def) { $r = Read-Host "$q [$def]"; if ([string]::IsNullOrWhiteSpace($r)) { $def } else { $r } }
function YesNo($q) { (Read-Host "$q [o/N]") -match '^[oOyY]$' }
function Has($cmd) { [bool](Get-Command $cmd -ErrorAction SilentlyContinue) }
function Refresh-Path {
  $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
              [Environment]::GetEnvironmentVariable("Path", "User")
}
function Check { if ($LASTEXITCODE -ne 0) { throw "Échec (code $LASTEXITCODE)" } }

# 1. Prérequis (via winget)
if (YesNo "Installer/vérifier les prérequis (Git, Node, Rust, VS Build Tools) ?") {
  if (-not (Has winget)) { throw "winget introuvable : installer 'App Installer' depuis le Microsoft Store." }
  if (-not (Has git))    { winget install -e --id Git.Git --accept-source-agreements --accept-package-agreements }
  if (-not (Has node))   { winget install -e --id OpenJS.NodeJS.LTS --accept-source-agreements --accept-package-agreements }
  if (-not (Has rustup)) { winget install -e --id Rustlang.Rustup --accept-source-agreements --accept-package-agreements }
  if (YesNo "Installer Visual Studio Build Tools (C++) ? (nécessaire une fois, ~5 Go)") {
    winget install -e --id Microsoft.VisualStudio.2022.BuildTools --accept-source-agreements --accept-package-agreements `
      --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  }
  Refresh-Path
  rustup default stable-msvc; Check
}
foreach ($c in "git", "node", "npm", "cargo") { if (-not (Has $c)) { throw "$c introuvable. Rouvrir le terminal ou installer les prérequis." } }

# 2. Clone ou pull
$Dir = Ask "Dossier du projet" "$HOME\Backseat"
$Branch = Ask "Branche" "main"
if (Test-Path "$Dir\.git") {
  Write-Host "==> Mise à jour de $Dir"
  git -C $Dir fetch origin $Branch; Check
  git -C $Dir checkout $Branch; Check
  git -C $Dir pull --ff-only origin $Branch; Check
} else {
  Write-Host "==> Clonage dans $Dir"
  git clone --branch $Branch $RepoUrl $Dir; Check
}
Set-Location $Dir
Write-Host "Rust : $(rustc --version) | Node : $(node --version)"

# 3. Dépendances JS
npm ci; if ($LASTEXITCODE -ne 0) { npm install; Check }

# 4. Mode
Write-Host "1) Lancer en dev (npm run dev)"
Write-Host "2) Build installeur NSIS (.exe)"
Write-Host "3) Build installeur MSI"
Write-Host "4) Build .exe + .msi"
switch (Ask "Choix" "4") {
  "1" { npm run dev; exit }
  "2" { $Bundles = "nsis" }
  "3" { $Bundles = "msi" }
  "4" { $Bundles = "nsis,msi" }
  default { throw "Choix invalide" }
}

npm run build -- --bundles $Bundles; Check

$Out = "src-tauri\target\release\bundle"
Write-Host "==> Terminé. Paquets :"
Get-ChildItem $Out -Recurse -Include *.exe, *.msi | ForEach-Object { $_.FullName }
Write-Host "Exe portable (sans installeur) : src-tauri\target\release\backseat.exe"
