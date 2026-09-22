# Timetable desktop build script (Windows).
# ASCII-only on purpose: Windows PowerShell 5.1 reads .ps1 as ANSI, so non-ASCII
# literals in this file corrupt parsing and eat quotes.
#
# Three environment quirks handled here:
#   1) global git has a dead proxy 127.0.0.1:7890 and cargo follows it -> clear proxy vars
#   2) the MSVC linker is not on PATH -> inject vcvars64.bat
#   3) a running timetable.exe locks target/release/timetable.exe, which makes the
#      link step fail with "failed to remove file ... os error 5" -> kill it first
#
# Usage: pwsh -File scripts/tauri-build.ps1 [-NoBundle] [-RunTests]

param(
  [switch]$NoBundle,
  [switch]$RunTests
)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$env:NODE_ENV = 'development'
$env:NO_PROXY = '*'; $env:no_proxy = '*'
$env:HTTP_PROXY = ''; $env:HTTPS_PROXY = ''; $env:ALL_PROXY = ''; $env:http_proxy = ''; $env:https_proxy = ''

# --- linker environment ---
$vcvars = Get-ChildItem "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" -ErrorAction SilentlyContinue
if ($vcvars) {
  cmd /c "`"$($vcvars.FullName)`" >nul 2>&1 && set" | ForEach-Object {
    if ($_ -match '^([^=]+)=(.*)$') { Set-Item -Force -Path "env:$($matches[1])" -Value $matches[2] }
  }
  Write-Host "vcvars injected; link.exe = $((Get-Command link.exe -ErrorAction SilentlyContinue).Source)"
} else {
  Write-Warning 'vcvars64.bat not found - Rust MSVC build will fail without link.exe'
}

# vcvars writes "Path"; PowerShell may keep PATH and Path as two separate variables
$bin = "$env:USERPROFILE\.cargo\bin"
$env:Path = "$bin;$env:Path"
$env:PATH = $env:Path

# --- 0) kill a running exe so the link step can replace it ---
Get-Process timetable -ErrorAction SilentlyContinue | ForEach-Object {
  Write-Host "stopping running timetable.exe (pid $($_.Id)) to unlock the binary"
  Stop-Process -Id $_.Id -Force
}
Start-Sleep -Milliseconds 500

# --- 1) frontend first: the desktop bundle embeds dist/, so it must exist and be complete ---
if (-not (Test-Path 'package.json')) { throw "not in project root: $root" }
Write-Host "`n=== npm run build (frontend) ==="
npm run build 2>&1 | Select-Object -Last 6
if ($LASTEXITCODE -ne 0) { throw "npm run build failed ($LASTEXITCODE)" }
if (-not (Test-Path 'dist/index.html')) { throw 'dist/index.html missing: frontend produced nothing, exe would embed empty assets' }
$entry = (Select-String -Path 'dist/index.html' -Pattern 'assets/index-[A-Za-z0-9_\-]+\.js' -AllMatches).Matches
if (-not $entry) { throw 'dist/index.html has no assets/index-*.js reference: incomplete build output' }
$entryName = [System.IO.Path]::GetFileName($entry[0].Value)
Write-Host "dist ok: index.html -> $entryName"

# --- 2) Rust unit tests ---
if ($RunTests) {
  Write-Host "`n=== cargo test --lib ==="
  & "$bin\cargo.exe" test --manifest-path src-tauri\Cargo.toml --lib 2>&1 | Select-Object -Last 20
  if ($LASTEXITCODE -ne 0) { throw "cargo test failed ($LASTEXITCODE)" }
}

# --- 3) bundle ---
$tauriArgs = @('tauri', 'build')
if ($NoBundle) { $tauriArgs += '--no-bundle' }

Write-Host "`n=== npx $($tauriArgs -join ' ') ==="
npx @tauriArgs 2>&1 | Select-Object -Last 15
$buildExit = $LASTEXITCODE
Write-Host "--- exit: $buildExit ---"
if ($buildExit -ne 0) { throw "tauri build failed ($buildExit)" }

# --- 4) verify the exe really embeds the dist we just built (otherwise it would hit devUrl) ---
$exe = 'src-tauri\target\release\timetable.exe'
if (Test-Path $exe) {
  $bytes = [System.IO.File]::ReadAllBytes((Resolve-Path $exe))
  $text = [System.Text.Encoding]::UTF8.GetString($bytes)
  $embedded = $text.Contains($entryName)
  Write-Host "verify: exe embeds $entryName -> $embedded"
  if (-not $embedded) { throw "exe does not embed $entryName (frontend assets missing, app would connect to devUrl)" }
} else {
  Write-Warning "$exe not found"
}

Get-ChildItem src-tauri\target\release\*.exe -ErrorAction SilentlyContinue | Select-Object Name, @{n = 'MB'; e = { [math]::Round($_.Length / 1MB, 1) } }
Get-ChildItem src-tauri\target\release\bundle -Recurse -File -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName
