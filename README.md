# MSGraphDBSynchronizer v2.1.0

A robust Microsoft Graph API database synchronization service with **dynamic multi-endpoint support**, advanced filtering, multi-database support, and comprehensive monitoring capabilities.

## 🚀 Features

### **🆕 New in v2.1.0**
- **🔄 SQLite WAL Mode**: Enhanced concurrent access support for Metabase and other BI tools
- **📅 Improved Timestamp Handling**: Proper timestamp column types across all databases for perfect sortability
- **🎯 Smart Field Detection**: Automatic timestamp field detection by name patterns and content
- **⚡ Performance Optimizations**: Enhanced WAL configuration with optimized cache settings
- **🔧 Enhanced Logging**: Better visibility into column type assignments and database operations

### **Previous Features (v2.0.0)**
- **🎯 Dynamic Endpoint Support**: Automatically sync any Microsoft Graph endpoint with configurable object counts
- **📊 Per-Endpoint Configuration**: Individual `mockObjectCount` settings for each endpoint
- **🔧 Realistic Mock Data**: Enterprise-grade test data with proper column mappings per endpoint
- **📱 Serial Number Device Names**: Real-world device naming using manufacturer-specific serial numbers
- **🏗️ Dynamic Schema Evolution**: Automatic table creation and column mapping based on endpoint data

### **Core Features**
- **🔄 Microsoft Graph Integration**: Sync any Graph API endpoint with OAuth2 authentication
- **🌐 Multi-Endpoint Support**: Sync devices, users, groups, compliance policies, and any custom endpoints
- **🎛️ Advanced OS Filtering**: Wildcard support with case-insensitive substring matching
- **💾 Multi-Database Support**: SQLite (WAL mode), PostgreSQL, and MSSQL backends with automatic schema creation
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
- **📊 BI Tool Ready**: SQLite WAL mode enables concurrent access for Metabase, Grafana, and other BI tools

## 📦 Quick Start

### Download Pre-built Binaries (Recommended)

1. **Download** the latest release for your platform:
   - [**Windows**](https://github.com/Grace-Solutions/MSGraphDBSynchronizer/releases/latest) - `*-windows-Release.zip`
   - [**Linux**](https://github.com/Grace-Solutions/MSGraphDBSynchronizer/releases/latest) - `*-linux-Release.zip`
   - [**macOS**](https://github.com/Grace-Solutions/MSGraphDBSynchronizer/releases/latest) - `*-macos-Release.zip`

2. **Extract** and configure:
   ```bash
   # Extract the package
   unzip MSGraphDBSynchronizer-*.zip
   cd MSGraphDBSynchronizer-*

   # Edit configuration with your Azure app details
   nano config.json  # or notepad config.json on Windows
   ```

3. **Run** the service:
   ```bash
   # Windows
   .\MSGraphDBSynchronizer.exe run

   # Linux/macOS
   ./MSGraphDBSynchronizer run
   ```

### Prerequisites

- Microsoft Azure App Registration with Intune permissions
- Database server (optional - SQLite included by default)
- Network access to Microsoft Graph API

### Build from Source

For building from source, see the [Build Guide](docs/BUILD.md).

## ⚙️ Configuration

### Quick Configuration

1. **Azure Setup**: Create an Azure App Registration with Microsoft Graph permissions
2. **Edit config.json**:
   ```json
   {
     "clientId": "your-azure-client-id",
     "clientSecret": "your-azure-client-secret",
     "tenantId": "your-azure-tenant-id",
     "pollInterval": "1h",
     "deviceOsFilter": ["Windows", "macOS", "Android", "iOS"],
     "database": {
       "sqlite": {
         "enabled": true,
         "databasePath": "./data/msgraph_data.db"
       }
     },
     "endpoints": {
       "endpoints": [
         {
           "name": "Devices",
           "enabled": true,
           "mockObjectCount": 30000,
           "selectFields": ["id", "deviceName", "operatingSystem", "serialNumber"]
         },
         {
           "name": "Users",
           "enabled": true,
           "mockObjectCount": 5000,
           "selectFields": ["id", "userPrincipalName", "displayName", "mail"]
         },
         {
           "name": "Groups",
           "enabled": true,
           "mockObjectCount": 1000,
           "selectFields": ["id", "displayName", "groupTypes", "securityEnabled"]
         }
       ]
     },
     "mockGraphApi": {
       "enabled": true
     }
   }
   ```

For detailed configuration options, see the [Configuration Guide](docs/CONFIGURATION.md).

## 🖥️ Service Management

### Windows Service
```bash
# Install and manage Windows service
MSGraphDBSynchronizer.exe install
MSGraphDBSynchronizer.exe start
MSGraphDBSynchronizer.exe status
```

### Linux/macOS
```bash
# Run in foreground
./MSGraphDBSynchronizer run

# Or install as systemd/launchd service (see Installation Guide)
```

### Configuration Validation
```bash
# Validate configuration before running
MSGraphDBSynchronizer.exe validate

# Validate specific config file
MSGraphDBSynchronizer.exe validate --config my-config.json
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
The service automatically creates tables for each enabled endpoint:
- **Devices** table - Device information with serial number device names
- **Users** table - User accounts with realistic names and departments
- **Groups** table - Security, Distribution, Microsoft 365, and Dynamic groups
- **Custom endpoints** - Any additional Graph API endpoints you configure

Each table has columns dynamically created based on the endpoint's `selectFields` configuration. Supports SQLite, PostgreSQL, and MSSQL with automatic schema creation and evolution.

### Database Compatibility & BI Tools

**SQLite WAL Mode** enables concurrent access for business intelligence tools:
- ✅ **Metabase**: Connect directly to SQLite database while service is running
- ✅ **Grafana**: Real-time dashboards with SQLite data source
- ✅ **Power BI**: Import data without service interruption
- ✅ **Tableau**: Live connections supported

**Timestamp Handling** ensures proper sorting across all databases:
- **SQLite**: `DATETIME` columns with ISO 8601 format
- **PostgreSQL**: `TIMESTAMPTZ` columns with timezone support
- **MSSQL**: `DATETIME2` columns with high precision

**Smart Field Detection** automatically identifies timestamp fields:
- Field names containing: `date`, `time`, `created`, `updated`, `modified`, `enrolled`, `last_sync`
- Field names ending with: `_at`, `_on`
- ISO 8601 timestamp content detection

## � Documentation

### Core Guides
- [**Installation Guide**](docs/INSTALLATION.md) - Platform-specific installation instructions
- [**Configuration Guide**](docs/CONFIGURATION.md) - Detailed configuration options and examples
- [**Build Guide**](docs/BUILD.md) - Building from source and cross-platform compilation

### Advanced Features
- [**Multi-Endpoint Support**](docs/ENDPOINTS.md) - Configure multiple Graph API endpoints with separate tables
- [**Rate Limiting**](docs/RATE_LIMITING.md) - API rate limiting and retry logic configuration
- [**Mock API**](docs/MOCK_API.md) - Testing with simulated Microsoft Graph API
- [**Config Validation**](docs/CONFIG_VALIDATION.md) - Configuration validation and troubleshooting
- [**Monitoring**](docs/monitoring/MONITORING.md) - Prometheus metrics and Grafana dashboards
- [**Troubleshooting**](docs/TROUBLESHOOTING.md) - Common issues and solutions

## 🐳 Docker

### Quick Start
```bash
# Build the image
docker build -t msgraph-db-sync .

# Run with data directory mount
docker run -d \
  -v $(pwd)/data:/app/data \
  -p 9898:9898 \
  --name msgraph-sync \
  msgraph-db-sync

# View logs
docker logs msgraph-sync

# Edit configuration
# The config file will be created at ./data/config.json on first run
# Edit it with your Azure credentials and restart the container
```

### Data Directory Structure
The container uses a single data directory mount that contains:
- `config.json` - Application configuration
- `msgraph_data.db` - SQLite database (if using SQLite backend)
- `logs/` - Application logs
- `backups/` - Database backups (if enabled)

### Docker Compose Example
```yaml
version: '3.8'
services:
  msgraph-sync:
    build: .
    ports:
      - "9898:9898"
    volumes:
      - ./data:/app/data
    restart: unless-stopped
    environment:
      - RUST_LOG=info
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