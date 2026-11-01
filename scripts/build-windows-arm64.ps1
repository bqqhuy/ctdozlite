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
if ($env:GITHUB_REF_TYPE -eq 'tag' -and $env:GITHUB_REF_NAME -ne "v$version") {
    throw "Release tag must match Cargo version: expected v$version"
}
New-Item -ItemType Directory -Force dist | Out-Null
# Start the real DX12 renderer with fixture data, capture pixels and require an exit.
# The fixture explicitly bypasses account restore and all network workers.
$previousData = $env:CTDOZ_DATA_DIR
$previousRenderer = $env:CTDOZ_RENDERER
try {
    $env:CTDOZ_RENDERER = 'dx12'
    foreach ($case in @(
        @{ Size='480x360'; Scale='1'; Tab='general' },
        @{ Size='1020x700'; Scale='1.5'; Tab='appearance' },
        @{ Size='1280x900'; Scale='2'; Tab='sync' }
    )) {
        $fixture = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
        $env:CTDOZ_DATA_DIR = $fixture
        $png = Join-Path (Resolve-Path dist) "ui-$($case.Size)-$($case.Scale)-$($case.Tab).png"
        $stdout = "$png.stdout.log"
        $stderr = "$png.stderr.log"
        $process = [Diagnostics.Process]::new()
        $process.StartInfo.FileName = $binary
        $process.StartInfo.UseShellExecute = $false
        $process.StartInfo.CreateNoWindow = $true
        $process.StartInfo.RedirectStandardOutput = $true
        $process.StartInfo.RedirectStandardError = $true
        foreach ($argument in @('--gui', '--dev', '--ui-smoke-test', $png, '--ui-smoke-size', $case.Size, '--ui-smoke-scale', $case.Scale, '--ui-smoke-tab', $case.Tab)) {
            $process.StartInfo.ArgumentList.Add($argument)
        }
        try {
            if (-not $process.Start()) { throw 'Cannot start GUI process' }
            $outTask = $process.StandardOutput.ReadToEndAsync()
            $errTask = $process.StandardError.ReadToEndAsync()
            if (-not $process.WaitForExit(45000)) {
                $process.Kill($true)
                $process.WaitForExit()
                throw "GUI did not exit within 45 seconds: $($case.Size)"
            }
            $outTask.GetAwaiter().GetResult() | Set-Content $stdout -Encoding utf8
            $errTask.GetAwaiter().GetResult() | Set-Content $stderr -Encoding utf8
            if ($process.ExitCode -ne 0 -or -not (Test-Path $png)) {
                Get-Content $stdout, $stderr -ErrorAction SilentlyContinue | Write-Host
                throw "GUI renderer smoke failed: $($case.Size), exit=$($process.ExitCode)"
            }
        } finally { $process.Dispose() }
        Remove-Item -Recurse -Force $fixture -ErrorAction SilentlyContinue
    }
} finally {
    $env:CTDOZ_DATA_DIR = $previousData
    $env:CTDOZ_RENDERER = $previousRenderer
}
Copy-Item $binary "dist\ctdozlite-$version-portable-windows-arm64.exe" -Force
$compiler = Get-Command iscc -ErrorAction SilentlyContinue
if (-not $compiler) { throw 'Install Inno Setup 6.7.1 or newer before packaging' }
$buildDir = Split-Path $binary
Invoke-Checked $compiler.Source @("/DMyAppVersion=$version", "/DBuildDir=$buildDir", 'packaging\windows\installer.iss')
$installer = 'dist\ctdozlite-setup-windows-arm64.exe'
if (-not (Test-Path $installer)) { throw 'Installer was not produced' }
Move-Item $installer "dist\ctdozlite-$version-setup-windows-arm64.exe" -Force
Copy-Item LICENSE-MIT, LICENSE-APACHE, THIRD_PARTY_NOTICES.txt, docs\WINDOWS_ARM64.md dist -Force
$sourceCommit = (& git rev-parse HEAD).Trim()
@{
    version = $version; target = $target; commit = $sourceCommit; rust = $hostInfo
    cargo_lock_sha256 = (Get-FileHash Cargo.lock -Algorithm SHA256).Hash.ToLowerInvariant()
    video_backend = 'Windows Media Foundation (native)'
    renderer = 'Direct3D 12'
    minimum_windows_build = 22000
    crt = 'static'
    signed = $false
} | ConvertTo-Json | Set-Content dist\build-info-windows-arm64.json -Encoding utf8
Get-ChildItem dist\*.exe | Sort-Object Name | ForEach-Object {
    "$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $($_.Name)"
} | Set-Content dist\SHA256SUMS-windows-arm64.txt -Encoding ascii
Get-ChildItem dist
