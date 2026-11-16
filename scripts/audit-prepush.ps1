# Pre-push integrity gate for ctdozlite.
#
# Every step reports one of:
#   CHECKED   - a hard gate that ran and passed (any failure makes the script exit 1)
#   FAILED    - a hard gate that ran and failed
#   SKIPPED   - not applicable here, with the reason
#   HEURISTIC - a regex scan; findings need human review and never prove UI completeness
#
# This script never creates tags, commits, pushes or releases.
#
# -Root       repository root to inspect (default: the parent of this script's folder)
# -SkipCargo  skip cargo check/clippy (used by the fixture tests of this script)
# -Tag        release tag to verify against; default is GITHUB_REF_NAME on a tag build,
#             otherwise an exact tag on HEAD, otherwise none (release sync is advisory)
param(
    [string]$Root = (Split-Path -Parent $PSScriptRoot),
    [switch]$SkipCargo,
    [string]$Tag
)

$ErrorActionPreference = "Stop"
Set-Location $Root
$results = [System.Collections.Generic.List[object]]::new()
function Add-Result([string]$Step, [string]$Status, [string]$Detail) {
    $results.Add([pscustomobject]@{ Step = $Step; Status = $Status; Detail = $Detail })
    $color = @{ CHECKED = "Green"; FAILED = "Red"; SKIPPED = "DarkGray"; HEURISTIC = "Yellow" }[$Status]
    Write-Host ("[{0}] {1}: {2}" -f $Status, $Step, $Detail) -ForegroundColor $color
}

# Mirrors src/updater.rs `version()`: legacy `v0.NNN[-arm]` is (0, 0, NNN); otherwise X.Y.Z.
function ConvertTo-VersionTuple([string]$Value) {
    $s = $Value -replace '^v', '' -replace '-arm$', ''
    if ($s -match '^0\.(\d+)$') { return @(0, 0, [int64]$Matches[1]) }
    if ($s -match '^(\d+)\.(\d+)\.(\d+)$') { return @([int64]$Matches[1], [int64]$Matches[2], [int64]$Matches[3]) }
    return $null
}

# ---- 1. Compilation and lint (hard) ----------------------------------------------------
if ($SkipCargo) {
    Add-Result "cargo check" "SKIPPED" "-SkipCargo"
    Add-Result "cargo clippy" "SKIPPED" "-SkipCargo"
} else {
    if (!(Get-Command cargo -ErrorAction SilentlyContinue) -and (Test-Path "$env:USERPROFILE\.cargo\bin\cargo.exe")) {
        $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    }
    if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
        Add-Result "cargo" "FAILED" "cargo not found in PATH or $env:USERPROFILE\.cargo\bin"
    } else {
        cargo check --locked
        if ($LASTEXITCODE -eq 0) { Add-Result "cargo check" "CHECKED" "cargo check --locked" }
        else { Add-Result "cargo check" "FAILED" "cargo check --locked exited $LASTEXITCODE" }
        cargo clippy --all-targets --locked -- -D warnings
        if ($LASTEXITCODE -eq 0) { Add-Result "cargo clippy" "CHECKED" "clippy --all-targets -D warnings" }
        else { Add-Result "cargo clippy" "FAILED" "clippy exited $LASTEXITCODE" }
    }
}

# ---- 2. UI stub scan (heuristic only) ------------------------------------------------
$stubHits = 0
if (Test-Path "src/gui") {
    Get-ChildItem -Path "src/gui" -Filter "*.rs" -Recurse | ForEach-Object {
        $content = Get-Content $_.FullName -Raw
        $found = [regex]::Matches($content, 'if\s+ui\.(?:button|small_button)\([^)]+\)\.clicked\(\)[^{]*\{([^}]+)\}')
        foreach ($m in $found) {
            $body = $m.Groups[1].Value
            if ($body -match 'status_msg\s*=' -and $body -notmatch 'WORKER_TX|WorkerCmd|tx\.send|self\.command') {
                Write-Warning "Review possible UI stub in $($_.Name): $($m.Value.Trim())"
                $stubHits++
            }
        }
    }
}
Add-Result "UI stub scan" "HEURISTIC" "$stubHits possible stub(s); regex review aid, not end-to-end UI verification"

# ---- 3. Four-point release synchronisation ---------------------------------------------
if (-not $PSBoundParameters.ContainsKey('Tag')) {
    if ($env:GITHUB_REF_TYPE -eq 'tag') { $Tag = $env:GITHUB_REF_NAME }
    else {
        # `--points-at` prints nothing (no stderr) when HEAD is untagged.
        $exact = @(git tag --points-at HEAD) | Where-Object { $_ -match '^v' } | Select-Object -First 1
        $Tag = if ($exact) { "$exact".Trim() } else { "" }
    }
}
$tagged = -not [string]::IsNullOrWhiteSpace($Tag)
# Untagged builds report release drift as advisory; a tag makes every mismatch fatal.
$mismatch = if ($tagged) { "FAILED" } else { "HEURISTIC" }
$suffix = if ($tagged) { "" } else { " (untagged: advisory, fails when tagged)" }

$libText = if (Test-Path "src/lib.rs") { Get-Content "src/lib.rs" -Raw } else { "" }
$product = [regex]::Match($libText, 'pub const PRODUCT_VERSION: &str = "([^"]+)";').Groups[1].Value
$tuple = if ($product) { ConvertTo-VersionTuple $product } else { $null }
if (-not $product -or -not $tuple) {
    Add-Result "PRODUCT_VERSION" "FAILED" "src/lib.rs has no parsable PRODUCT_VERSION ('$product')"
} else {
    $semver = "{0}.{1}.{2}" -f $tuple
    Add-Result "PRODUCT_VERSION" "CHECKED" "$product -> ($($tuple -join ', '))"

    if ($tagged) {
        if ($Tag -eq $product) { Add-Result "tag" "CHECKED" "tag $Tag == PRODUCT_VERSION" }
        else { Add-Result "tag" "FAILED" "tag '$Tag' != PRODUCT_VERSION '$product'" }
    } else {
        Add-Result "tag" "SKIPPED" "HEAD has no release tag"
    }

    $updater = if (Test-Path "src/updater.rs") { Get-Content "src/updater.rs" -Raw } else { "" }
    $expect = [regex]::Match($updater, 'assert_eq!\(\s*version\(crate::PRODUCT_VERSION\),\s*Some\(\((\d+),\s*(\d+),\s*(\d+)\)\)\s*\)')
    if (-not $expect.Success) {
        Add-Result "updater regression" $mismatch "src/updater.rs lacks the PRODUCT_VERSION tuple assertion$suffix"
    } elseif (((@(1, 2, 3) | ForEach-Object { [int64]($expect.Groups[$_].Value) }) -join '.') -ne $semver) {
        Add-Result "updater regression" $mismatch "updater expects ($($expect.Groups[1].Value), $($expect.Groups[2].Value), $($expect.Groups[3].Value)), PRODUCT_VERSION is ($($tuple -join ', '))$suffix"
    } else {
        Add-Result "updater regression" "CHECKED" "expects ($($tuple -join ', '))"
    }

    $cargoToml = if (Test-Path "Cargo.toml") { Get-Content "Cargo.toml" -Raw } else { "" }
    $package = [regex]::Match($cargoToml, '(?ms)^\[package\].*?^name\s*=\s*"([^"]+)".*?^version\s*=\s*"([^"]+)"')
    if (-not $package.Success) {
        Add-Result "Cargo.toml" $mismatch "no [package] name/version$suffix"
    } else {
        $name = $package.Groups[1].Value
        $tomlVersion = $package.Groups[2].Value
        if ($tomlVersion -eq $semver) { Add-Result "Cargo.toml" "CHECKED" "version $tomlVersion" }
        else { Add-Result "Cargo.toml" $mismatch "version $tomlVersion != $semver$suffix" }

        $lock = if (Test-Path "Cargo.lock") { Get-Content "Cargo.lock" -Raw } else { "" }
        $locked = [regex]::Match($lock, "(?m)^name = `"$([regex]::Escape($name))`"\r?\nversion = `"([^`"]+)`"")
        if (-not $locked.Success) { Add-Result "Cargo.lock" $mismatch "package $name not found$suffix" }
        elseif ($locked.Groups[1].Value -ne $semver) { Add-Result "Cargo.lock" $mismatch "$name $($locked.Groups[1].Value) != $semver$suffix" }
        else { Add-Result "Cargo.lock" "CHECKED" "$name $semver" }
    }

    $changelog = if (Test-Path "CHANGELOG.md") { Get-Content "CHANGELOG.md" -Raw } else { "" }
    if ($changelog -match "(?m)^## $([regex]::Escape($product))(\s|$)") { Add-Result "CHANGELOG" "CHECKED" "has '## $product'" }
    else { Add-Result "CHANGELOG" $mismatch "no '## $product' header$suffix" }
}

# ---- Summary ---------------------------------------------------------------------------
$failed = @($results | Where-Object Status -eq "FAILED")
$counts = ($results | Group-Object Status | ForEach-Object { "$($_.Name)=$($_.Count)" }) -join ", "
Write-Host "`nSummary: $counts"
if ($failed.Count -gt 0) {
    Write-Host ">>> Pre-push gate FAILED: $($failed.Step -join ', ')" -ForegroundColor Red
    exit 1
}
Write-Host ">>> Hard gates passed. SKIPPED and HEURISTIC items above are not verified." -ForegroundColor Green
exit 0
