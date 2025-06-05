#!/usr/bin/env pwsh
<#
.SYNOPSIS
    GitHub release script for IntuneDeviceDatabaseSynchronization
.DESCRIPTION
    This script builds cross-platform binaries, creates a GitHub release,
    and uploads all platform-specific packages plus the sample config.
.PARAMETER Version
    Version tag for the release (e.g., "v1.0.0"). If not provided, uses timestamp.
.PARAMETER PreRelease
    Mark the release as a pre-release
.PARAMETER Draft
    Create as a draft release
.PARAMETER SkipBuild
    Skip the build process and use existing artifacts
.EXAMPLE
    .\release.ps1
    .\release.ps1 -Version "v1.2.3"
    .\release.ps1 -PreRelease -Draft
#>

param(
    [string]$Version,
    [switch]$PreRelease,
    [switch]$Draft,
    [switch]$SkipBuild
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Project information
$ProjectName = "MSGraphDBSynchronizer"

Write-Host "GitHub Release for $ProjectName" -ForegroundColor Green

# Check if gh CLI is available
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Error "GitHub CLI (gh) is not installed. Please install it from https://cli.github.com/"
    exit 1
}

# Check if we're in a git repository
if (-not (Test-Path ".git")) {
    Write-Error "Not in a git repository. Please run this script from the project root."
    exit 1
}

# Generate version if not provided
if (-not $Version) {
    $Now = Get-Date
    $Version = "v{0}.{1:D2}.{2:D2}.{3:D2}{4:D2}" -f $Now.Year, $Now.Month, $Now.Day, $Now.Hour, $Now.Minute
    Write-Host "Generated version: $Version" -ForegroundColor Yellow
} else {
    Write-Host "Using version: $Version" -ForegroundColor Yellow
}

# Ensure we have the latest changes
Write-Host "Checking git status..." -ForegroundColor Blue
$GitStatus = & git status --porcelain
if ($GitStatus) {
    Write-Warning "There are uncommitted changes:"
    Write-Host $GitStatus -ForegroundColor Yellow
    $Continue = Read-Host "Continue anyway? (y/N)"
    if ($Continue -ne "y" -and $Continue -ne "Y") {
        Write-Host "Aborted by user" -ForegroundColor Red
        exit 1
    }
}

try {
    # Build cross-platform binaries unless skipped
    if (-not $SkipBuild) {
        Write-Host "Building cross-platform binaries..." -ForegroundColor Blue
        & .\build-cross-platform.ps1 -Configuration Release
        if ($LASTEXITCODE -ne 0) {
            throw "Cross-platform build failed"
        }
    } else {
        Write-Host "Skipping build (using existing artifacts)" -ForegroundColor Yellow
    }

    # Verify build artifacts exist
    $DistDir = ".\dist"
    if (-not (Test-Path $DistDir)) {
        throw "Distribution directory not found. Run build first."
    }

    # Find all platform packages
    $Packages = Get-ChildItem -Path $DistDir -Filter "*.zip" | Where-Object { $_.Name -like "*Release.zip" }
    if ($Packages.Count -eq 0) {
        throw "No release packages found in $DistDir"
    }

    Write-Host "Found packages:" -ForegroundColor Green
    foreach ($Package in $Packages) {
        Write-Host "  $($Package.Name)" -ForegroundColor White
    }

    # Copy standalone config file (not zipped)
    Write-Host "Creating standalone configuration file..." -ForegroundColor Blue
    $StandaloneConfigPath = Join-Path $DistDir "config.json"
    if (Test-Path "config.json") {
        Copy-Item "config.json" $StandaloneConfigPath -Force
        Write-Host "  Standalone config: $StandaloneConfigPath" -ForegroundColor Green
    }

    # Commit and tag if there are changes
    Write-Host "Creating git tag..." -ForegroundColor Blue
    
    # Check if tag already exists
    $ExistingTag = & git tag -l $Version
    if ($ExistingTag) {
        Write-Warning "Tag $Version already exists"
        $Overwrite = Read-Host "Delete and recreate tag? (y/N)"
        if ($Overwrite -eq "y" -or $Overwrite -eq "Y") {
            & git tag -d $Version
            & git push origin --delete $Version 2>$null
        } else {
            Write-Host "Using existing tag" -ForegroundColor Yellow
        }
    }

    if (-not $ExistingTag -or ($Overwrite -eq "y" -or $Overwrite -eq "Y")) {
        & git tag -a $Version -m "Release $Version"
        & git push origin $Version
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to push tag to origin"
        }
    }

    # Generate release notes
    $ReleaseNotes = @"
# $ProjectName $Version

## Features

- Microsoft Intune device synchronization with OS filtering
- Multi-database support (SQLite, PostgreSQL, MSSQL)
- Prometheus metrics and monitoring
- Cross-platform support (Windows, Linux, macOS)
- Native service management
- Automatic versioning and build system

## Downloads

Choose the appropriate package for your platform:

- **Windows**: $ProjectName-*-windows-Release.zip
- **Linux**: $ProjectName-*-linux-Release.zip
- **macOS**: $ProjectName-*-macos-Release.zip
- **Configuration**: config.json (sample configuration file)

## Installation

1. Download the appropriate package for your platform
2. Extract the package to your desired location
3. Configure the application using config.json
4. Run the service (see documentation for platform-specific instructions)

## Documentation

- Installation Guide (docs/INSTALLATION.md)
- Configuration Guide (docs/CONFIGURATION.md)
- Build Guide (docs/BUILD.md)
- Troubleshooting Guide (docs/TROUBLESHOOTING.md)

## Requirements

- Microsoft Azure App Registration with Intune permissions
- Database server (optional - SQLite included)
- Network access to Microsoft Graph API

## What's New

- Cross-platform binary distribution
- Enhanced build system with automatic versioning
- Comprehensive documentation
- Improved error handling and logging
- Backup/restore functionality for SQLite databases
- Webhook support for real-time notifications
- Grafana monitoring dashboard examples

For detailed installation and configuration instructions, see the included documentation.
"@

    # Create GitHub release
    Write-Host "Creating GitHub release..." -ForegroundColor Blue
    
    $ReleaseArgs = @("release", "create", $Version)
    
    if ($PreRelease) {
        $ReleaseArgs += "--prerelease"
    }
    
    if ($Draft) {
        $ReleaseArgs += "--draft"
    }
    
    $ReleaseArgs += "--title"
    $ReleaseArgs += "$ProjectName $Version"
    
    $ReleaseArgs += "--notes"
    $ReleaseArgs += $ReleaseNotes

    # Add all packages and standalone config
    $AllPackages = @($Packages)
    foreach ($Package in $AllPackages) {
        $ReleaseArgs += $Package.FullName
    }

    # Add standalone config file
    if (Test-Path $StandaloneConfigPath) {
        $ReleaseArgs += $StandaloneConfigPath
    }

    & gh @ReleaseArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to create GitHub release"
    }

    Write-Host ""
    Write-Host "GitHub release created successfully!" -ForegroundColor Green
    Write-Host "Version: $Version" -ForegroundColor White
    Write-Host "Packages uploaded: $($AllPackages.Count)" -ForegroundColor White
    Write-Host "Config file uploaded: config.json" -ForegroundColor White
    
    # Show release URL
    $RepoUrl = & git config --get remote.origin.url
    if ($RepoUrl) {
        $RepoUrl = $RepoUrl -replace "\.git$", ""
        $RepoUrl = $RepoUrl -replace "git@github\.com:", "https://github.com/"
        $ReleaseUrl = "$RepoUrl/releases/tag/$Version"
        Write-Host "Release URL: $ReleaseUrl" -ForegroundColor Cyan
    } else {
        Write-Host "Release URL: https://github.com/Grace-Solutions/MSGraphDBSynchronizer/releases/tag/$Version" -ForegroundColor Cyan
    }

} catch {
    Write-Host "Release failed: $_" -ForegroundColor Red
    exit 1
} finally {
    # No cleanup needed for standalone config file
}
