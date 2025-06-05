# Build Guide

## Prerequisites

### Required Tools

- **Rust**: 1.75 or later
- **Git**: For source code management
- **PowerShell**: 5.1 or later (for automated builds)

### Platform-Specific Requirements

#### Windows
- **Windows SDK**: For Windows resource embedding
- **Visual Studio Build Tools**: For MSVC toolchain
- **winres**: Automatically installed via Cargo

#### Linux
- **GCC**: For compilation
- **pkg-config**: For library detection
- **libssl-dev**: For TLS support
- **libpq-dev**: For PostgreSQL support (optional)

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install build-essential pkg-config libssl-dev libpq-dev

# CentOS/RHEL/Fedora
sudo dnf install gcc pkg-config openssl-devel postgresql-devel
```

#### macOS
- **Xcode Command Line Tools**: `xcode-select --install`
- **Homebrew**: For additional dependencies

```bash
# Install Homebrew if not present
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install pkg-config openssl postgresql
```

## Build Methods

### Method 1: Automated Cross-Platform Build (Recommended)

The project includes automated build scripts that handle cross-platform compilation:

```powershell
# Build for all platforms (Windows, Linux, macOS)
.\build-cross-platform.ps1

# Build for specific platforms
.\build-cross-platform.ps1 -Platforms @("windows", "linux")

# Debug build
.\build-cross-platform.ps1 -Configuration Debug

# Skip tests
.\build-cross-platform.ps1 -SkipTests
```

**Features**:
- ✅ Automatic version generation (timestamp-based)
- ✅ Cross-platform binary compilation
- ✅ Windows resource embedding (icon, version info)
- ✅ Distribution package creation
- ✅ Test execution
- ✅ Build metadata tracking

### Method 2: Platform-Specific Build

#### Windows Build

```powershell
# Simple build
.\build.ps1

# With options
.\build.ps1 -Configuration Release -OutputDir "C:\Builds"

# Or use batch wrapper
.\build.bat
```

#### Manual Cargo Build

```bash
# Clone repository
git clone https://github.com/Grace-Solutions/MSGraphDBSynchronizer.git
cd MSGraphDBSynchronizer

# Build for current platform
cargo build --release

# Build for specific target
cargo build --release --target x86_64-pc-windows-msvc
```

## Cross-Platform Compilation

### Installing Rust Targets

```bash
# Windows target (from Linux/macOS)
rustup target add x86_64-pc-windows-msvc

# Linux target (from Windows/macOS)  
rustup target add x86_64-unknown-linux-gnu

# macOS target (from Windows/Linux)
rustup target add x86_64-apple-darwin
```

### Manual Cross-Compilation

```bash
# Build for Windows
cargo build --release --target x86_64-pc-windows-msvc

# Build for Linux
cargo build --release --target x86_64-unknown-linux-gnu

# Build for macOS
cargo build --release --target x86_64-apple-darwin
```

**Note**: Cross-compilation to macOS from other platforms requires additional setup and may not work reliably. Use the automated build script on macOS for best results.

## Build Artifacts

### Directory Structure

After building, the following structure is created:

```
dist/
├── windows/
│   ├── MSGraphDBSynchronizer.exe
│   ├── version.json
│   ├── config.json
│   ├── README.md
│   └── docs/
├── linux/
│   ├── MSGraphDBSynchronizer
│   ├── version.json
│   ├── config.json
│   ├── README.md
│   └── docs/
├── macos/
│   ├── MSGraphDBSynchronizer
│   ├── version.json
│   ├── config.json
│   ├── README.md
│   └── docs/
├── MSGraphDBSynchronizer-VERSION-windows-Release.zip
├── MSGraphDBSynchronizer-VERSION-linux-Release.zip
└── MSGraphDBSynchronizer-VERSION-macos-Release.zip
```

### Version Information

Each build includes:
- **Timestamp-based version**: `yyyy.MM.dd.HHmm`
- **Build metadata**: Machine, user, Git commit
- **Platform information**: Target architecture and OS
- **Binary size**: File size information

Example `version.json`:
```json
{
  "ProductName": "IntuneDeviceDatabaseSynchronization",
  "Version": "2025.06.02.2215",
  "BuildTimestamp": "2025-06-02 22:15:30 UTC",
  "Configuration": "Release",
  "Platform": "windows",
  "Target": "x86_64-pc-windows-msvc",
  "BuildMachine": "BUILD-SERVER",
  "BuildUser": "builder",
  "GitCommit": "a1b2c3d4e5f6...",
  "BinaryPath": "IntuneDeviceDatabaseSynchronization.exe",
  "BinarySize": 14166528
}
```

## Build Configuration

### Cargo.toml Features

The project uses conditional compilation for platform-specific features:

```toml
[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["winuser", "winsvc"] }

[build-dependencies]
chrono = "0.4"
winres = "0.1"  # Windows resource embedding
```

### Build Script (build.rs)

The build script handles:
- Version generation from timestamp
- Windows resource embedding
- Icon embedding (Windows only)
- Build metadata generation

## Troubleshooting

### Common Build Issues

#### 1. Missing Rust Targets

**Error**: `error: toolchain 'stable-x86_64-pc-windows-msvc' is not installed`

**Solution**:
```bash
rustup target add x86_64-pc-windows-msvc
```

#### 2. Windows Resource Compilation Errors

**Error**: `Failed to compile Windows resources`

**Solutions**:
- Ensure Windows SDK is installed
- Check that `assets/icon.ico` exists
- Verify winres dependency is available

#### 3. Database Driver Compilation Issues

**Error**: `failed to run custom build command for 'libpq-sys'`

**Solutions**:
```bash
# Ubuntu/Debian
sudo apt install libpq-dev

# macOS
brew install postgresql

# Windows
# Use bundled features or install PostgreSQL
```

#### 4. SSL/TLS Compilation Issues

**Error**: `failed to run custom build command for 'openssl-sys'`

**Solutions**:
```bash
# Ubuntu/Debian
sudo apt install libssl-dev

# macOS
brew install openssl
export OPENSSL_DIR=$(brew --prefix openssl)

# Windows
# Usually resolved by using rustls instead of native-tls
```

### Build Performance

#### Optimizing Build Times

1. **Use Cargo Cache**:
   ```bash
   # Install sccache for distributed compilation caching
   cargo install sccache
   export RUSTC_WRAPPER=sccache
   ```

2. **Parallel Compilation**:
   ```bash
   # Set number of parallel jobs
   export CARGO_BUILD_JOBS=8
   ```

3. **Incremental Builds**:
   ```bash
   # Enable incremental compilation (default in debug)
   export CARGO_INCREMENTAL=1
   ```

#### Release Optimization

The release builds use these optimizations:

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
```

## Continuous Integration

### GitHub Actions Example

```yaml
name: Build and Release

on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        os: [windows-latest, ubuntu-latest, macos-latest]
    
    runs-on: ${{ matrix.os }}
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true
    
    - name: Build
      run: cargo build --release
    
    - name: Upload artifacts
      uses: actions/upload-artifact@v3
      with:
        name: ${{ matrix.os }}-binary
        path: target/release/MSGraphDBSynchronizer*
```

For complete CI/CD setup, see the `.github/workflows/` directory in the repository.
