#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Automated build script for IntuneDeviceDatabaseSynchronization with version management
.DESCRIPTION
    This script automatically builds the project with timestamp-based versioning,
    embeds the icon and version information, and creates release artifacts.
.PARAMETER Configuration
    Build configuration (Debug or Release). Default: Release
.PARAMETER SkipTests
    Skip running tests before building
.PARAMETER OutputDir
    Output directory for build artifacts. Default: .\dist
.EXAMPLE
    .\build.ps1
    .\build.ps1 -Configuration Debug
    .\build.ps1 -SkipTests -OutputDir "C:\Builds"
#>

param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    
    [switch]$SkipTests,
    
    [string]$OutputDir = ".\dist"
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Get project information
$ProjectName = "IntuneDeviceDatabaseSynchronization"
$ProjectRoot = $PSScriptRoot

Write-Host "Building $ProjectName" -ForegroundColor Green
Write-Host "Configuration: $Configuration" -ForegroundColor Cyan
Write-Host "Output Directory: $OutputDir" -ForegroundColor Cyan

# Generate version based on current timestamp
$Now = Get-Date
$Version = "{0}.{1:D2}.{2:D2}.{3:D2}{4:D2}" -f $Now.Year, $Now.Month, $Now.Day, $Now.Hour, $Now.Minute
$BuildTimestamp = $Now.ToString("yyyy-MM-dd HH:mm:ss UTC")

Write-Host "Version: $Version" -ForegroundColor Yellow
Write-Host "Build Time: $BuildTimestamp" -ForegroundColor Yellow

# Update Cargo.toml with new version (for SemVer compatibility, we'll use a different approach)
$CargoTomlPath = Join-Path $ProjectRoot "Cargo.toml"
$CargoContent = Get-Content $CargoTomlPath -Raw

# Create a backup
$BackupPath = "$CargoTomlPath.backup"
Copy-Item $CargoTomlPath $BackupPath

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

    # Check if icon exists
    $IconPath = Join-Path $ProjectRoot "assets\icon.ico"
    if (-not (Test-Path $IconPath)) {
        Write-Warning "Icon not found at $IconPath. Icon will not be embedded."
    } else {
        Write-Host "Icon found: $IconPath" -ForegroundColor Green
    }

    # Build the project
    Write-Host "Building $ProjectName..." -ForegroundColor Blue
    $BuildArgs = @("build")
    if ($Configuration -eq "Release") {
        $BuildArgs += "--release"
    }
    
    & cargo @BuildArgs
    if ($LASTEXITCODE -ne 0) { throw "Build failed" }

    # Create output directory
    if (-not (Test-Path $OutputDir)) {
        New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
    }

    # Determine binary path
    $BinaryName = "$ProjectName.exe"
    $SourcePath = if ($Configuration -eq "Release") {
        Join-Path $ProjectRoot "target\release\$BinaryName"
    } else {
        Join-Path $ProjectRoot "target\debug\$BinaryName"
    }

    if (-not (Test-Path $SourcePath)) {
        throw "Built binary not found at $SourcePath"
    }

    # Copy binary to output directory
    $OutputBinaryPath = Join-Path $OutputDir $BinaryName
    Copy-Item $SourcePath $OutputBinaryPath -Force
    Write-Host "Binary copied to: $OutputBinaryPath" -ForegroundColor Green

    # Get file version info
    if (Test-Path $OutputBinaryPath) {
        $FileInfo = Get-Item $OutputBinaryPath
        $FileVersion = $FileInfo.VersionInfo.FileVersion
        $ProductVersion = $FileInfo.VersionInfo.ProductVersion

        Write-Host "File Information:" -ForegroundColor Cyan
        Write-Host "   Size: $([math]::Round($FileInfo.Length / 1MB, 2)) MB" -ForegroundColor White
        Write-Host "   File Version: $FileVersion" -ForegroundColor White
        Write-Host "   Product Version: $ProductVersion" -ForegroundColor White
    }

    # Create version info file
    $VersionInfoPath = Join-Path $OutputDir "version.json"
    $VersionInfo = @{
        ProductName = $ProjectName
        Version = $Version
        BuildTimestamp = $BuildTimestamp
        Configuration = $Configuration
        BuildMachine = $env:COMPUTERNAME
        BuildUser = $env:USERNAME
        GitCommit = if (Get-Command git -ErrorAction SilentlyContinue) { 
            try { & git rev-parse HEAD 2>$null } catch { "unknown" }
        } else { "unknown" }
        BinaryPath = $OutputBinaryPath
        BinarySize = (Get-Item $OutputBinaryPath).Length
    } | ConvertTo-Json -Depth 2

    Set-Content -Path $VersionInfoPath -Value $VersionInfo -Encoding UTF8
    Write-Host "Version info saved to: $VersionInfoPath" -ForegroundColor Green

    # Create ZIP package for distribution
    $ZipPath = Join-Path $OutputDir "$ProjectName-$Version-$Configuration.zip"
    $TempDir = Join-Path $env:TEMP "$ProjectName-package"
    
    if (Test-Path $TempDir) { Remove-Item $TempDir -Recurse -Force }
    New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
    
    # Copy files to package
    Copy-Item $OutputBinaryPath $TempDir
    Copy-Item $VersionInfoPath $TempDir
    
    # Copy additional files if they exist
    $AdditionalFiles = @("README.md", "LICENSE", "config.json", ".env.example")
    foreach ($File in $AdditionalFiles) {
        $FilePath = Join-Path $ProjectRoot $File
        if (Test-Path $FilePath) {
            Copy-Item $FilePath $TempDir
        }
    }
    
    # Create ZIP
    Compress-Archive -Path "$TempDir\*" -DestinationPath $ZipPath -Force
    Remove-Item $TempDir -Recurse -Force

    Write-Host "Package created: $ZipPath" -ForegroundColor Green

    Write-Host ""
    Write-Host "Build completed successfully!" -ForegroundColor Green
    Write-Host "   Binary: $OutputBinaryPath" -ForegroundColor White
    Write-Host "   Package: $ZipPath" -ForegroundColor White
    Write-Host "   Version: $Version" -ForegroundColor White

} catch {
    Write-Host "Build failed: $_" -ForegroundColor Red
    exit 1
} finally {
    # Restore Cargo.toml backup
    if (Test-Path $BackupPath) {
        Move-Item $BackupPath $CargoTomlPath -Force
    }
}
