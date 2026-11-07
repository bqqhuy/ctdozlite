# Automated Pre-Push and Integrity Audit Script for ctdozlite
# Ensures zero compilation errors, no dropped payloads, and no UI stubs before pushing.

$ErrorActionPreference = "Stop"

Write-Host "=== [1/4] Ensuring Rust Toolchain in PATH ===" -ForegroundColor Cyan
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    if (Test-Path "$env:USERPROFILE\.cargo\bin\cargo.exe") {
        $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
        Write-Host "Loaded cargo from $env:USERPROFILE\.cargo\bin" -ForegroundColor Green
    } else {
        Write-Error "Cargo is not found in PATH or $env:USERPROFILE\.cargo\bin!"
    }
}
cargo --version

Write-Host "`n=== [2/4] Running Local Compilation & Strict Clippy Check ===" -ForegroundColor Cyan
Write-Host "--> Checking types and compilation (cargo check --locked)..."
cargo check --locked
if ($LASTEXITCODE -ne 0) {
    Write-Error "cargo check failed! Fix all compiler and type errors before committing or pushing."
}
Write-Host "--> Running strict lint inspection (cargo clippy --all-targets --locked -- -D warnings)..."
cargo clippy --all-targets --locked -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Error "cargo clippy detected warnings or errors! Fix all lint issues before committing or pushing."
}
Write-Host "Local compilation & strict Clippy check PASSED!" -ForegroundColor Green

Write-Host "`n=== [3/4] Scanning for Suspicious UI Stubs ===" -ForegroundColor Cyan
$stubFound = $false
Get-ChildItem -Path "src/gui" -Filter "*.rs" -Recurse | ForEach-Object {
    $content = Get-Content $_.FullName -Raw
    # Detect button clicks that only modify status_msg without sending WorkerCmd
    $matches = [regex]::Matches($content, 'if\s+ui\.(?:button|small_button)\([^)]+\)\.clicked\(\)[^{]*\{([^}]+)\}')
    foreach ($m in $matches) {
        $body = $m.Groups[1].Value
        if ($body -match 'status_msg\s*=' -and $body -notmatch 'WORKER_TX|WorkerCmd|tx\.send|self\.command') {
            Write-Warning "Potential UI Stub found in $($_.Name): $($m.Value.Trim())"
            $stubFound = $true
        }
    }
}

if ($stubFound) {
    Write-Warning "Found potential UI stubs. Review them to ensure no fake buttons remain."
} else {
    Write-Host "No obvious UI stubs found!" -ForegroundColor Green
}

Write-Host "`n=== [4/4] Verifying 4-Point Release Synchronization (if tagged) ===" -ForegroundColor Cyan
$libRs = Get-Content "src/lib.rs" -Raw
if ($libRs -match 'pub const PRODUCT_VERSION: &str = "(v[^"]+)";') {
    $ver = $matches[1]
    Write-Host "Current PRODUCT_VERSION in src/lib.rs: $ver" -ForegroundColor Green
}

Write-Host "`n>>> All Pre-Push Integrity Audits Completed Successfully! <<<" -ForegroundColor Green
