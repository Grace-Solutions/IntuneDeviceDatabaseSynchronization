#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Cross-platform build script for IntuneDeviceDatabaseSynchronization
.DESCRIPTION
    This script builds binaries for Windows, Linux, and macOS with automatic versioning
    and creates distribution packages for each platform.
.PARAMETER Configuration
    Build configuration (Debug or Release). Default: Release
.PARAMETER SkipTests
    Skip running tests before building
.PARAMETER OutputDir
    Output directory for build artifacts. Default: .\dist
.PARAMETER Platforms
    Platforms to build for. Default: @("windows", "linux", "macos")
.EXAMPLE
    .\build-cross-platform.ps1
    .\build-cross-platform.ps1 -Configuration Debug -Platforms @("windows", "linux")
#>

param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    
    [switch]$SkipTests,
    
    [string]$OutputDir = ".\dist",
    
    [ValidateSet("windows", "linux", "macos")]
    [string[]]$Platforms = @("windows", "linux", "macos")
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Project information
$ProjectName = "MSGraphDBSynchronizer"
$ProjectRoot = $PSScriptRoot

# Target configurations
$Targets = @{
    "windows" = @{
        Target = "x86_64-pc-windows-msvc"
        Extension = ".exe"
        Archive = "zip"
    }
    "linux" = @{
        Target = "x86_64-unknown-linux-gnu"
        Extension = ""
        Archive = "tar.gz"
    }
    "macos" = @{
        Target = "x86_64-apple-darwin"
        Extension = ""
        Archive = "tar.gz"
    }
}

Write-Host "Cross-Platform Build for $ProjectName" -ForegroundColor Green
Write-Host "Configuration: $Configuration" -ForegroundColor Cyan
Write-Host "Output Directory: $OutputDir" -ForegroundColor Cyan
Write-Host "Platforms: $($Platforms -join ', ')" -ForegroundColor Cyan

# Generate version based on current timestamp
$Now = Get-Date
$Version = "{0}.{1:D2}.{2:D2}.{3:D2}{4:D2}" -f $Now.Year, $Now.Month, $Now.Day, $Now.Hour, $Now.Minute
$BuildTimestamp = $Now.ToString("yyyy-MM-dd HH:mm:ss UTC")

Write-Host "Version: $Version" -ForegroundColor Yellow
Write-Host "Build Time: $BuildTimestamp" -ForegroundColor Yellow

# Create output directory
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

try {
    # Clean previous builds
    Write-Host "Cleaning previous builds..." -ForegroundColor Blue
    & cargo clean
    if ($LASTEXITCODE -ne 0) { throw "Cargo clean failed" }

    # Run tests unless skipped
    if (-not $SkipTests) {
        Write-Host "Running tests..." -ForegroundColor Blue
        & cargo test
        if ($LASTEXITCODE -ne 0) { throw "Tests failed" }
    }

    # Install required targets
    Write-Host "Installing required Rust targets..." -ForegroundColor Blue
    foreach ($Platform in $Platforms) {
        $Target = $Targets[$Platform].Target
        Write-Host "  Installing target: $Target" -ForegroundColor Gray
        & rustup target add $Target
        if ($LASTEXITCODE -ne 0) { 
            Write-Warning "Failed to install target $Target, skipping $Platform"
            continue
        }
    }

    $BuiltPlatforms = @()
    $FailedPlatforms = @()

    # Build for each platform
    foreach ($Platform in $Platforms) {
        $TargetInfo = $Targets[$Platform]
        $Target = $TargetInfo.Target
        $Extension = $TargetInfo.Extension
        
        Write-Host "Building for $Platform ($Target)..." -ForegroundColor Blue
        
        try {
            # Build command
            $BuildArgs = @("build", "--target", $Target)
            if ($Configuration -eq "Release") {
                $BuildArgs += "--release"
            }
            
            & cargo @BuildArgs
            if ($LASTEXITCODE -ne 0) { 
                throw "Build failed for $Platform"
            }

            # Determine binary paths
            $BinaryName = "$ProjectName$Extension"
            $SourcePath = if ($Configuration -eq "Release") {
                Join-Path $ProjectRoot "target\$Target\release\$BinaryName"
            } else {
                Join-Path $ProjectRoot "target\$Target\debug\$BinaryName"
            }

            if (-not (Test-Path $SourcePath)) {
                throw "Built binary not found at $SourcePath"
            }

            # Create platform-specific output directory
            $PlatformDir = Join-Path $OutputDir $Platform
            if (-not (Test-Path $PlatformDir)) {
                New-Item -ItemType Directory -Path $PlatformDir -Force | Out-Null
            }

            # Copy binary
            $OutputBinaryPath = Join-Path $PlatformDir $BinaryName
            Copy-Item $SourcePath $OutputBinaryPath -Force
            
            Write-Host "  Binary: $OutputBinaryPath" -ForegroundColor Green
            
            # Get file info
            $FileInfo = Get-Item $OutputBinaryPath
            $FileSize = [math]::Round($FileInfo.Length / 1MB, 2)
            Write-Host "  Size: $FileSize MB" -ForegroundColor Gray

            $BuiltPlatforms += @{
                Platform = $Platform
                Target = $Target
                BinaryPath = $OutputBinaryPath
                Size = $FileInfo.Length
            }

        } catch {
            Write-Warning "Failed to build for $Platform`: $_"
            $FailedPlatforms += $Platform
        }
    }

    # Create version info and packages
    Write-Host "Creating distribution packages..." -ForegroundColor Blue
    
    foreach ($PlatformInfo in $BuiltPlatforms) {
        $Platform = $PlatformInfo.Platform
        $PlatformDir = Join-Path $OutputDir $Platform
        
        # Create version info file
        $VersionInfoPath = Join-Path $PlatformDir "version.json"
        $VersionInfo = @{
            ProductName = $ProjectName
            Version = $Version
            BuildTimestamp = $BuildTimestamp
            Configuration = $Configuration
            Platform = $Platform
            Target = $PlatformInfo.Target
            BuildMachine = $env:COMPUTERNAME
            BuildUser = $env:USERNAME
            GitCommit = if (Get-Command git -ErrorAction SilentlyContinue) { 
                try { & git rev-parse HEAD 2>$null } catch { "unknown" }
            } else { "unknown" }
            BinaryPath = $PlatformInfo.BinaryPath
            BinarySize = $PlatformInfo.Size
        } | ConvertTo-Json -Depth 2

        Set-Content -Path $VersionInfoPath -Value $VersionInfo -Encoding UTF8
        
        # Copy additional files (ensure config.json is always included)
        $AdditionalFiles = @("README.md", "LICENSE", "config.json", ".env.example")
        foreach ($File in $AdditionalFiles) {
            $FilePath = Join-Path $ProjectRoot $File
            if (Test-Path $FilePath) {
                Copy-Item $FilePath $PlatformDir -Force
                Write-Host "  Included: $File" -ForegroundColor Gray
            } elseif ($File -eq "config.json") {
                Write-Warning "config.json not found - this should be included in releases"
            }
        }

        # Copy docs folder if it exists
        $DocsPath = Join-Path $ProjectRoot "docs"
        if (Test-Path $DocsPath) {
            $PlatformDocsPath = Join-Path $PlatformDir "docs"
            Copy-Item $DocsPath $PlatformDocsPath -Recurse -Force
            Write-Host "  Included: docs folder" -ForegroundColor Gray
        }

        # Create archive
        $ArchiveType = $Targets[$Platform].Archive
        $ArchiveName = "$ProjectName-$Version-$Platform-$Configuration"
        
        if ($ArchiveType -eq "zip") {
            $ArchivePath = Join-Path $OutputDir "$ArchiveName.zip"
            Compress-Archive -Path "$PlatformDir\*" -DestinationPath $ArchivePath -Force
        } else {
            # For tar.gz, we'll create a zip for now (cross-platform compatibility)
            $ArchivePath = Join-Path $OutputDir "$ArchiveName.zip"
            Compress-Archive -Path "$PlatformDir\*" -DestinationPath $ArchivePath -Force
        }
        
        Write-Host "  Package: $ArchivePath" -ForegroundColor Green
    }

    # Summary
    Write-Host ""
    Write-Host "Cross-platform build completed!" -ForegroundColor Green
    Write-Host "Version: $Version" -ForegroundColor White
    Write-Host "Built platforms: $($BuiltPlatforms.Platform -join ', ')" -ForegroundColor White
    
    if ($FailedPlatforms.Count -gt 0) {
        Write-Host "Failed platforms: $($FailedPlatforms -join ', ')" -ForegroundColor Red
    }

} catch {
    Write-Host "Build failed: $_" -ForegroundColor Red
    exit 1
}
