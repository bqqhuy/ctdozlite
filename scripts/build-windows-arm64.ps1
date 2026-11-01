param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Set-Location (Join-Path $PSScriptRoot '..')
$target = 'aarch64-pc-windows-msvc'
$env:CMAKE_POLICY_VERSION_MINIMUM = '3.5'
function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed with exit code $LASTEXITCODE" }
}
function Assert-Arm64([string]$Path) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) { throw "Invalid DOS header: $Path" }
    $offset = [BitConverter]::ToInt32($bytes, 0x3C)
    if ($offset -lt 64 -or $offset + 6 -gt $bytes.Length) { throw "Invalid PE offset: $Path" }
    if ([BitConverter]::ToUInt32($bytes, $offset) -ne 0x4550) { throw "Invalid PE signature: $Path" }
    $machine = [BitConverter]::ToUInt16($bytes, $offset + 4)
    if ($machine -ne 0xAA64) { throw "Expected native ARM64 PE, got 0x$('{0:X4}' -f $machine): $Path" }
    Write-Host "ARM64 PE verified: $Path"
}
function Get-Dumpbin {
    $tool = Get-Command dumpbin -ErrorAction SilentlyContinue
    if ($tool) { return $tool.Source }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { throw 'Visual Studio vswhere is required to inspect DLL dependencies' }
    foreach ($hostArch in @('Hostarm64', 'Hostx64')) {
        $paths = @(& $vswhere -latest -products '*' -find "VC\Tools\MSVC\*\bin\$hostArch\arm64\dumpbin.exe")
        if ($LASTEXITCODE -eq 0 -and $paths.Count -gt 0 -and (Test-Path $paths[0])) { return $paths[0] }
    }
    throw 'ARM64 MSVC dumpbin was not found'
}
$hostInfo = (& rustc -vV) -join "`n"
if ($LASTEXITCODE -ne 0 -or $hostInfo -notmatch 'host: aarch64-pc-windows-msvc') { throw 'Use native Rust ARM64 MSVC on Windows ARM64' }
if (-not $SkipTests) { Invoke-Checked cargo @('test', '--locked', '--all-targets', '--target', $target) }
Invoke-Checked cargo @('build', '--locked', '--release', '--bin', 'zalo-rust', '--target', $target)
$binary = (Resolve-Path "target\$target\release\zalo-rust.exe").Path
Assert-Arm64 $binary
$dumpbin = Get-Dumpbin
$imports = & $dumpbin /DEPENDENTS $binary
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect DLL imports' }
$imports | Write-Host
if ($imports -match '(?i)(?:VCRUNTIME|MSVCP)\w*\.dll|(?:opus|sqlite3)\.dll') {
    throw 'Portable ARM64 executable depends on a separately installed runtime DLL'
}
Invoke-Checked $binary @('--version')
$version = ((& cargo metadata --locked --no-deps --format-version 1) | ConvertFrom-Json).packages |
    Where-Object { $_.name -eq 'ctdozlite' } | Select-Object -ExpandProperty version
if ($LASTEXITCODE -ne 0 -or -not $version) { throw 'Cannot read package version' }
New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item $binary "dist\ctdozlite-$version-portable-windows-arm64.exe" -Force
$compiler = Get-Command iscc -ErrorAction SilentlyContinue
if (-not $compiler) { throw 'Install Inno Setup 6.7.1 or newer before packaging' }
$buildDir = Split-Path $binary
Invoke-Checked $compiler.Source @('/DArch=arm64', "/DMyAppVersion=$version", "/DBuildDir=$buildDir", 'packaging\windows\installer.iss')
$installer = 'dist\ctdozlite-setup-windows-arm64.exe'
if (-not (Test-Path $installer)) { throw 'Installer was not produced' }
Move-Item $installer "dist\ctdozlite-$version-setup-windows-arm64.exe" -Force
Copy-Item LICENSE-MIT, LICENSE-APACHE, docs\WINDOWS_ARM64.md dist -Force
$sourceCommit = (& git rev-parse HEAD).Trim()
@{
    version = $version; target = $target; commit = $sourceCommit; rust = $hostInfo
    cargo_lock_sha256 = (Get-FileHash Cargo.lock -Algorithm SHA256).Hash.ToLowerInvariant()
    ffmpeg = 'External native ARM64 dependency; run zalo-rust.exe --doctor'
    crt = 'static'
    signed = $false
} | ConvertTo-Json | Set-Content dist\build-info-windows-arm64.json -Encoding utf8
Get-ChildItem dist\*.exe | Sort-Object Name | ForEach-Object {
    "$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $($_.Name)"
} | Set-Content dist\SHA256SUMS-windows-arm64.txt -Encoding ascii
Get-ChildItem dist
