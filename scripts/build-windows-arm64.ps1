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
if (-not $SkipTests) {
    Invoke-Checked cargo @('test', '--locked', '--all-targets', '--target', $target)
    # Public codec only; no account/session/phone connection is used by this test.
    $codecPath = Join-Path ([IO.Path]::GetTempPath()) ('ctdoz-codec-' + [guid]::NewGuid().ToString() + '.wasm')
    $previousCodec = $env:CTDOZ_CODEC_TEST_PATH
    try {
        $source = Get-Content src/endpoints.rs -Raw
        $codecUrl = [regex]::Match($source, 'PHONE_SYNC_CODEC: &str =\s*"(https://[^" ]+)"').Groups[1].Value
        if (-not $codecUrl) { throw 'Pinned codec URL missing' }
        Invoke-WebRequest -Uri $codecUrl -OutFile $codecPath -TimeoutSec 60
        $codecBytes = [IO.File]::ReadAllBytes($codecPath)
        if ($codecBytes.Length -ge 2 -and $codecBytes[0] -eq 0x1f -and $codecBytes[1] -eq 0x8b) {
            $inputStream = [IO.MemoryStream]::new($codecBytes)
            $gzip = [IO.Compression.GZipStream]::new($inputStream, [IO.Compression.CompressionMode]::Decompress)
            $output = [IO.MemoryStream]::new()
            try {
                $buffer = [byte[]]::new(8192)
                while (($read = $gzip.Read($buffer, 0, $buffer.Length)) -gt 0) {
                    if ($output.Length + $read -gt 1048576) { throw 'Codec exceeds size limit' }
                    $output.Write($buffer, 0, $read)
                }
                [IO.File]::WriteAllBytes($codecPath, $output.ToArray())
            } finally { $gzip.Dispose(); $inputStream.Dispose(); $output.Dispose() }
        }
        $expected = [regex]::Match((Get-Content src/phone_sync/assets.rs -Raw), 'CODEC_SHA256: &str =\s*"([a-f0-9]{64})"').Groups[1].Value
        if (-not $expected -or (Get-FileHash -Algorithm SHA256 $codecPath).Hash.ToLowerInvariant() -ne $expected) { throw 'Official codec hash mismatch' }
        $env:CTDOZ_CODEC_TEST_PATH = $codecPath
        Invoke-Checked cargo @('test', '--locked', '--lib', '--target', $target, 'phone_sync::codec', '--', '--ignored')
    } finally {
        $env:CTDOZ_CODEC_TEST_PATH = $previousCodec
        Remove-Item $codecPath -Force -ErrorAction SilentlyContinue
    }
}
Invoke-Checked cargo @('build', '--locked', '--release', '--bin', 'ctdozlite', '--target', $target)
$binary = (Resolve-Path "target\$target\release\ctdozlite.exe").Path
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
# Inspect embedded resources from the finished ARM64 PE, not source strings.
$properties = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($binary)
if ($properties.ProductVersion -ne $version -or $properties.OriginalFilename -ne 'ctdozlite.exe') {
    throw 'Embedded version resource does not match the package'
}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class NativeManifestResources {
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern IntPtr LoadLibraryExW(string path, IntPtr file, uint flags);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern IntPtr FindResourceW(IntPtr module, IntPtr name, IntPtr type);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern IntPtr LoadResource(IntPtr module, IntPtr resource);
    [DllImport("kernel32.dll")] public static extern IntPtr LockResource(IntPtr resource);
    [DllImport("kernel32.dll")] public static extern uint SizeofResource(IntPtr module, IntPtr resource);
    [DllImport("kernel32.dll")] public static extern bool FreeLibrary(IntPtr module);
}
'@
$module = [NativeManifestResources]::LoadLibraryExW($binary, [IntPtr]::Zero, 0x22)
if ($module -eq [IntPtr]::Zero) { throw 'Cannot load PE resources for inspection' }
try {
    $resource = [NativeManifestResources]::FindResourceW($module, [IntPtr]1, [IntPtr]24)
    if ($resource -eq [IntPtr]::Zero) { throw 'ARM64 application manifest is missing' }
    $size = [NativeManifestResources]::SizeofResource($module, $resource)
    $data = [NativeManifestResources]::LockResource([NativeManifestResources]::LoadResource($module, $resource))
    if ($size -eq 0 -or $data -eq [IntPtr]::Zero) { throw 'Cannot read application manifest' }
    $manifest = [System.Runtime.InteropServices.Marshal]::PtrToStringUTF8($data, [int]$size)
    [xml]$manifestXml = $manifest
    if ($manifestXml.assembly.assemblyIdentity.processorArchitecture -ne 'arm64' -or $manifest -notmatch 'level="asInvoker"') {
        throw 'Application manifest must declare ARM64 and standard-user privileges'
    }
    Write-Host "ARM64 resources verified: version=$version, asInvoker"
} finally { [void][NativeManifestResources]::FreeLibrary($module) }

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
        @{ Size='1280x900'; Scale='2'; Tab='sync' },
        @{ Size='800x600'; Scale='1'; Tab='general'; Tray=$true },
        @{ Size='480x360'; Scale='1'; Tab='workspace-sync'; WorkspaceSync=$true },
        @{ Size='1020x700'; Scale='1.5'; Tab='workspace-sync'; WorkspaceSync=$true },
        @{ Size='480x360'; Scale='1.5'; Tab='phone-sync'; PhoneSync=$true },
        @{ Size='1020x700'; Scale='1.5'; Tab='quote-jump'; QuoteJump=$true },
        @{ Size='1020x700'; Scale='1.5'; Tab='photo'; Photo=$true },
        @{ Size='1020x700'; Scale='1'; Tab='media'; Media=$true },
        @{ Size='1020x700'; Scale='1'; Tab='contacts'; Contacts=$true },
        @{ Size='1020x700'; Scale='1'; Tab='schedule'; Schedule=$true },
        @{ Size='480x360'; Scale='1'; Tab='schedule-small'; Schedule=$true },
        @{ Size='1020x700'; Scale='1'; Tab='ami'; Ami=$true },
        @{ Size='1020x700'; Scale='1'; Tab='rich-text'; RichText=$true },
        @{ Size='1020x700'; Scale='1.5'; Tab='outgoing'; Outgoing=$true },
        @{ Size='1020x700'; Scale='1'; Tab='tools-0'; ToolsTab=0 },
        @{ Size='1020x700'; Scale='1'; Tab='tools-1'; ToolsTab=1 },
        @{ Size='1020x700'; Scale='1'; Tab='tools-2'; ToolsTab=2 },
        @{ Size='1020x700'; Scale='1'; Tab='tools-3'; ToolsTab=3 },
        @{ Size='1020x700'; Scale='1'; Tab='tools-4'; ToolsTab=4 }
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
        if ($case.ContainsKey('Tray')) { $process.StartInfo.ArgumentList.Add('--tray-smoke-test') }
        if ($case.ContainsKey('WorkspaceSync')) { $process.StartInfo.ArgumentList.Add('--ui-smoke-workspace-sync') }
        if ($case.ContainsKey('PhoneSync')) { $process.StartInfo.ArgumentList.Add('--ui-smoke-phone-sync') }
        if ($case.ContainsKey('ToolsTab')) { $process.StartInfo.ArgumentList.Add('--ui-smoke-tools'); $process.StartInfo.ArgumentList.Add('--ui-smoke-tools-tab'); $process.StartInfo.ArgumentList.Add([string]$case.ToolsTab) }
        if ($case.ContainsKey('Outgoing')) { $process.StartInfo.ArgumentList.Add('--ui-smoke-outgoing') }
        if ($case.ContainsKey('Photo')) { $process.StartInfo.ArgumentList.Add('--ui-smoke-photo') }
        if ($case.ContainsKey('Media')) { $process.StartInfo.ArgumentList.Add('--ui-smoke-media') }
        if ($case.ContainsKey('Contacts')) { $process.StartInfo.ArgumentList.Add('--ui-smoke-contacts') }
        if ($case.ContainsKey('Schedule')) { $process.StartInfo.ArgumentList.Add('--ui-smoke-schedule') }
        if ($case.ContainsKey('Ami')) { $process.StartInfo.ArgumentList.Add('--ui-smoke-ami') }
        if ($case.ContainsKey('RichText')) { $process.StartInfo.ArgumentList.Add('--ui-smoke-rich-text') }
        if ($case.ContainsKey('QuoteJump')) { $process.StartInfo.ArgumentList.Add('--ui-smoke-quote-jump') }
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
