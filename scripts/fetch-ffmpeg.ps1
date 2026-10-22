<#
.SYNOPSIS
Downloads the pinned LGPL FFmpeg 8.1 shared build (BtbN/FFmpeg-Builds),
verifies its size and SHA-256 and unpacks it (M2-01, VAL-MEDIA-004).

.DESCRIPTION
Windows counterpart of scripts/fetch-ffmpeg (same pins, same layout).

Default destination: <checkout>\.amp\in\ffmpeg\n8.1.3-lgpl, the development
folder the locator reads in debug builds (ignored by git; never commit it).
For the installer, pass -Dest <staging>\ffmpeg; the installer copies its
bin folder to <install dir>\ffmpeg\bin.

Nothing is unpacked unless the archive has the pinned size and SHA-256. The
archive is downloaded to a temporary folder next to the destination and
deleted after unpacking unless -KeepArchive is given. A destination that
already holds this build (SELA-PIN.txt matches) is left alone unless -Force.

Exit codes: 0 ready, 1 download/verification/unpack failure, 2 usage.
Policy and obligations: docs/media.md.

.EXAMPLE
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\fetch-ffmpeg.ps1
#>
[CmdletBinding()]
param(
    [ValidateSet('win64', 'linux64')]
    [string]$Platform = 'win64',
    [string]$Dest = '',
    [switch]$KeepArchive,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Tag = 'autobuild-2026-09-30-13-08'
$Version = 'n8.1.3-9-g29e619e767'
# SELA_FFMPEG_BASE_URL replaces the download folder (a mirror, or file:// in
# tests); the size and SHA-256 pins still apply.
$Base = if ($env:SELA_FFMPEG_BASE_URL) { $env:SELA_FFMPEG_BASE_URL } else {
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/$Tag"
}
$SourceCommit = '29e619e767'

switch ($Platform) {
    'win64' {
        $Asset = "ffmpeg-$Version-win64-lgpl-shared-8.1.zip"
        $Size = 80733362
        $Sha256 = '3e47bda1607740550141e37c0e49d1e5182b34699f15adfd137ee266d346811a'
    }
    'linux64' {
        $Asset = "ffmpeg-$Version-linux64-lgpl-shared-8.1.tar.xz"
        $Size = 65590572
        $Sha256 = 'd4d7b6936f492c0b866b1cb4a29a1bbae09d6a064b4ec4eb90c95bed7969535a'
    }
}

$Root = Split-Path -Parent $PSScriptRoot
if (-not $Dest) {
    $Dest = Join-Path $Root '.amp\in\ffmpeg\n8.1.3-lgpl'
}
$Dest = [System.IO.Path]::GetFullPath($Dest)

$PinText = @(
    "source=$Base/$Asset",
    "tag=$Tag",
    "asset=$Asset",
    "size=$Size",
    "sha256=$Sha256",
    "version=$Version",
    'license=LGPL-3.0-or-later',
    "source_commit=$SourceCommit (FFmpeg release/8.1)"
) -join "`n"

$Pin = Join-Path $Dest 'SELA-PIN.txt'
if (-not $Force -and (Test-Path -LiteralPath $Pin)) {
    $existing = (Get-Content -LiteralPath $Pin -Raw).TrimEnd("`r", "`n")
    if ($existing -eq $PinText) {
        Write-Output "fetch-ffmpeg: $Dest already holds $Asset"
        exit 0
    }
}

$Parent = Split-Path -Parent $Dest
New-Item -ItemType Directory -Force -Path $Parent | Out-Null
$Work = Join-Path $Parent (".fetch-ffmpeg." + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $Work | Out-Null
try {
    $Archive = Join-Path $Work $Asset
    Write-Output "fetch-ffmpeg: downloading $Base/$Asset"
    try {
        $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -Uri "$Base/$Asset" -OutFile $Archive -UseBasicParsing
    } catch {
        Write-Error "fetch-ffmpeg: download failed: $_" -ErrorAction Continue
        exit 1
    }

    $ActualSize = (Get-Item -LiteralPath $Archive).Length
    if ($ActualSize -ne $Size) {
        Write-Error "fetch-ffmpeg: size mismatch: expected $Size bytes, got $ActualSize; nothing unpacked" -ErrorAction Continue
        exit 1
    }
    $ActualSha = (Get-FileHash -LiteralPath $Archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ActualSha -ne $Sha256) {
        Write-Error "fetch-ffmpeg: SHA-256 mismatch: expected $Sha256, got $ActualSha; nothing unpacked" -ErrorAction Continue
        exit 1
    }
    Write-Output "fetch-ffmpeg: size and SHA-256 verified ($Sha256)"

    $Unpacked = Join-Path $Work 'unpacked'
    New-Item -ItemType Directory -Path $Unpacked | Out-Null
    if ($Asset.EndsWith('.zip')) {
        Expand-Archive -LiteralPath $Archive -DestinationPath $Unpacked
        $Top = Join-Path $Unpacked ($Asset -replace '\.zip$', '')
    } else {
        tar -xJf $Archive -C $Unpacked
        if ($LASTEXITCODE -ne 0) {
            Write-Error 'fetch-ffmpeg: tar failed' -ErrorAction Continue
            exit 1
        }
        $Top = Join-Path $Unpacked ($Asset -replace '\.tar\.xz$', '')
    }
    if (-not (Test-Path -LiteralPath (Join-Path $Top 'bin'))) {
        Write-Error "fetch-ffmpeg: unexpected archive layout (no $(Split-Path -Leaf $Top)\bin)" -ErrorAction Continue
        exit 1
    }
    [System.IO.File]::WriteAllText((Join-Path $Top 'SELA-PIN.txt'), "$PinText`n")

    if ($KeepArchive) {
        Move-Item -LiteralPath $Archive -Destination (Join-Path $Parent $Asset) -Force
        Write-Output "fetch-ffmpeg: kept $(Join-Path $Parent $Asset)"
    }
    $Old = "$Dest.old"
    if (Test-Path -LiteralPath $Old) { Remove-Item -LiteralPath $Old -Recurse -Force }
    if (Test-Path -LiteralPath $Dest) { Move-Item -LiteralPath $Dest -Destination $Old }
    Move-Item -LiteralPath $Top -Destination $Dest
    if (Test-Path -LiteralPath $Old) { Remove-Item -LiteralPath $Old -Recurse -Force }
    Write-Output "fetch-ffmpeg: unpacked $Asset to $Dest"
    exit 0
} finally {
    if (Test-Path -LiteralPath $Work) { Remove-Item -LiteralPath $Work -Recurse -Force }
}
