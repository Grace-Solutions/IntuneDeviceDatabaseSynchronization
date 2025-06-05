# IntuneDeviceDatabaseSynchronization

A robust Microsoft Intune device synchronization service with advanced OS filtering, multi-database support, and comprehensive monitoring capabilities.

## 🚀 Features

- **🔄 Microsoft Intune Integration**: Sync device data from Microsoft Graph API with OAuth2 authentication
- **🎛️ Advanced OS Filtering**: Wildcard support with case-insensitive substring matching
- **💾 Multi-Database Support**: SQLite, PostgreSQL, and MSSQL backends with automatic schema creation
- **📊 Prometheus Metrics**: Comprehensive monitoring and observability
- **🖥️ Cross-Platform**: Native binaries for Windows, Linux, and macOS
- **🛠️ Service Management**: Windows service, systemd, and launchd support
- **⚙️ Flexible Configuration**: JSON config with environment variable overrides
- **🔍 Smart Change Detection**: Hash-based updates to avoid unnecessary database writes
- **📝 Structured Logging**: Component-based logging with rotation and configurable levels
- **🐳 Container Ready**: Docker support with multi-stage builds
- **🚦 Rate Limiting**: Intelligent API rate limiting with exponential backoff retry logic
- **🧪 Mock API**: Complete Graph API simulation for testing and development
- **✅ Config Validation**: Comprehensive configuration validation with detailed error reporting
- **💾 Backup & Restore**: Automated SQLite database backups with retention policies
- **🔔 Webhook Notifications**: Real-time event notifications for external integrations

## 📦 Quick Start

### Download Pre-built Binaries (Recommended)

1. **Download** the latest release for your platform:
   - [**Windows**](https://github.com/Grace-Solutions/IntuneDeviceDatabaseSynchronization/releases/latest) - `*-windows-Release.zip`
   - [**Linux**](https://github.com/Grace-Solutions/IntuneDeviceDatabaseSynchronization/releases/latest) - `*-linux-Release.zip`
   - [**macOS**](https://github.com/Grace-Solutions/IntuneDeviceDatabaseSynchronization/releases/latest) - `*-macos-Release.zip`

2. **Extract** and configure:
   ```bash
   # Extract the package
   unzip IntuneDeviceDatabaseSynchronization-*.zip
   cd IntuneDeviceDatabaseSynchronization-*

   # Edit configuration with your Azure app details
   nano config.json  # or notepad config.json on Windows
   ```

3. **Run** the service:
   ```bash
   # Windows
   .\IntuneDeviceDatabaseSynchronization.exe run

   # Linux/macOS
   ./IntuneDeviceDatabaseSynchronization run
   ```

### Prerequisites

- Microsoft Azure App Registration with Intune permissions
- Database server (optional - SQLite included by default)
- Network access to Microsoft Graph API

### Build from Source

For building from source, see the [Build Guide](docs/BUILD.md).

## ⚙️ Configuration

### Quick Configuration

1. **Azure Setup**: Create an Azure App Registration with Intune permissions
2. **Edit config.json**:
   ```json
   {
     "clientId": "your-azure-client-id",
     "clientSecret": "your-azure-client-secret",
     "tenantId": "your-azure-tenant-id",
     "pollInterval": "1h",
     "deviceOsFilter": ["Windows", "macOS", "Android"],
     "database": {
       "backends": ["sqlite"],
       "sqlitePath": "./output/devices.db"
     }
   }
   ```

For detailed configuration options, see the [Configuration Guide](docs/CONFIGURATION.md).

## 🖥️ Service Management

### Windows Service
```bash
# Install and manage Windows service
IntuneDeviceDatabaseSynchronization.exe install
IntuneDeviceDatabaseSynchronization.exe start
IntuneDeviceDatabaseSynchronization.exe status
```

### Linux/macOS
```bash
# Run in foreground
./IntuneDeviceDatabaseSynchronization run

# Or install as systemd/launchd service (see Installation Guide)
```

### Configuration Validation
```bash
# Validate configuration before running
IntuneDeviceDatabaseSynchronization.exe validate

# Validate specific config file
IntuneDeviceDatabaseSynchronization.exe validate --config my-config.json
```

For detailed installation instructions, see the [Installation Guide](docs/INSTALLATION.md).

## 📊 Monitoring

### Prometheus Metrics
Access metrics at `http://localhost:9898/metrics`

Key metrics include:
- Sync operations (success/failure/duration)
- Device counts and filtering statistics
- Database operations and errors
- Authentication and HTTP metrics

### Database Schema
The service automatically creates:
- **devices** table - Main device information
- **device_metadata** table - Additional unmapped fields

Supports SQLite, PostgreSQL, and MSSQL with automatic schema creation.

## � Documentation

### Core Guides
- [**Installation Guide**](docs/INSTALLATION.md) - Platform-specific installation instructions
- [**Configuration Guide**](docs/CONFIGURATION.md) - Detailed configuration options and examples
- [**Build Guide**](docs/BUILD.md) - Building from source and cross-platform compilation

### Advanced Features
- [**Rate Limiting**](docs/RATE_LIMITING.md) - API rate limiting and retry logic configuration
- [**Mock API**](docs/MOCK_API.md) - Testing with simulated Microsoft Graph API
- [**Config Validation**](docs/CONFIG_VALIDATION.md) - Configuration validation and troubleshooting
- [**Monitoring**](docs/monitoring/MONITORING.md) - Prometheus metrics and Grafana dashboards
- [**Troubleshooting**](docs/TROUBLESHOOTING.md) - Common issues and solutions

## 🐳 Docker

```bash
docker build -t intune-device-sync .
docker run -d \
  -v $(pwd)/config.json:/app/config.json \
  -v $(pwd)/logs:/app/logs \
  -p 9898:9898 \
  intune-device-sync
```

## � Development

```bash
# Run tests
cargo test

# Debug mode
RUST_LOG=debug cargo run -- run

# Cross-platform build
.\build-cross-platform.ps1
```

## 🚀 Releases

Create releases with the automated script:
```bash
# Build and create GitHub release
.\release.ps1

# Create pre-release
.\release.ps1 -PreRelease
```

## � License

This project is licensed under the GPL-3.0 License - see the [LICENSE](LICENSE) file for details.

---

<!-- AUGMENT NOTES FOR FUTURE CHANGES:
✅ COMPLETED FEATURES:
- ✅ Rate limiting with exponential backoff retry logic
- ✅ Mock Graph API for testing and development
- ✅ Configuration validation with detailed error reporting
- ✅ Backup/restore functionality for SQLite databases
- ✅ Webhook support for real-time notifications
- ✅ Grafana monitoring dashboard examples
- ✅ Comprehensive error handling examples in documentation

🔮 FUTURE ENHANCEMENTS:
- Consider adding GitHub Actions workflow for automated builds and releases
- Add performance tuning guide for large environments
- Consider adding serverless deployment options (Azure Functions, AWS Lambda)
- Add multi-cloud support and edge computing capabilities
- Consider adding anomaly detection for device patterns
- Add automated remediation for common issues
- Consider adding SIEM integration (Splunk, QRadar)
- Add business intelligence connectors and reporting engine
-->