# Configuration Guide

## Overview

IntuneDeviceDatabaseSynchronization supports configuration through:
1. **JSON configuration file** (`config.json`)
2. **Environment variables** (override JSON settings)
3. **Command-line arguments** (for service management)

## Configuration File Structure

The main configuration is stored in `config.json`:

```json
{
  "clientId": "your-azure-client-id",
  "clientSecret": "your-azure-client-secret", 
  "tenantId": "your-azure-tenant-id",
  "pollInterval": "1h",
  "cronSchedule": null,
  "deviceOsFilter": ["Windows", "macOS", "Android", "iOS"],
  "enablePrometheus": true,
  "prometheusPort": 9898,
  "logLevel": "info",
  "database": {
    "backends": ["sqlite"],
    "tableName": "devices",
    "sqlitePath": "./output/devices.db",
    "postgres": {
      "connectionString": "postgres://user:password@localhost:5432/intune_devices"
    },
    "mssql": {
      "connectionString": "server=localhost;database=intune_devices;trusted_connection=true"
    }
  }
}
```

## Configuration Options

### Authentication Settings

| Setting | Type | Required | Description |
|---------|------|----------|-------------|
| `clientId` | string | Yes | Azure App Registration Client ID |
| `clientSecret` | string | Yes | Azure App Registration Client Secret |
| `tenantId` | string | Yes | Azure Tenant ID |

### Sync Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `pollInterval` | string | "1h" | Sync interval (e.g., "30m", "2h", "1d") |
| `cronSchedule` | string | null | Cron expression for scheduling (overrides pollInterval) |

**Poll Interval Examples**:
- `"30s"` - Every 30 seconds
- `"5m"` - Every 5 minutes  
- `"1h"` - Every hour
- `"12h"` - Every 12 hours
- `"1d"` - Every day

**Cron Schedule Examples**:
- `"0 */6 * * *"` - Every 6 hours
- `"0 9 * * 1-5"` - 9 AM on weekdays
- `"0 0 * * 0"` - Every Sunday at midnight

### Device Filtering

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `deviceOsFilter` | array | `["*"]` | OS types to sync |

**Filter Options**:
- `["*"]` - All devices (wildcard)
- `["Windows"]` - Windows devices only
- `["Windows", "macOS"]` - Windows and macOS devices
- `["Android", "iOS"]` - Mobile devices only

**Supported OS Types**:
- `Windows`
- `macOS` 
- `Android`
- `iOS`
- `Linux` (if supported by Intune)

### Monitoring Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `enablePrometheus` | boolean | true | Enable Prometheus metrics |
| `prometheusPort` | number | 9898 | Metrics server port |
| `logLevel` | string | "info" | Log level (trace, debug, info, warn, error) |

### Database Configuration

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `backends` | array | `["sqlite"]` | Database backends to use |
| `tableName` | string | "devices" | Main table name |

#### SQLite Configuration

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `sqlitePath` | string | "./output/devices.db" | SQLite database file path |

#### PostgreSQL Configuration

| Setting | Type | Required | Description |
|---------|------|----------|-------------|
| `connectionString` | string | Yes | PostgreSQL connection string |

**Connection String Format**:
```
postgres://username:password@hostname:port/database_name
postgresql://username:password@hostname:port/database_name?sslmode=require
```

#### MSSQL Configuration

| Setting | Type | Required | Description |
|---------|------|----------|-------------|
| `connectionString` | string | Yes | MSSQL connection string |

**Connection String Formats**:
```
# Windows Authentication
server=localhost;database=intune_devices;trusted_connection=true

# SQL Server Authentication  
server=localhost;database=intune_devices;uid=username;pwd=password

# With encryption
server=localhost;database=intune_devices;uid=username;pwd=password;encrypt=true;trustServerCertificate=true
```

## Environment Variables

All configuration options can be overridden using environment variables with the `INTUNE_` prefix:

| Environment Variable | Configuration Path |
|---------------------|-------------------|
| `INTUNE_CLIENT_ID` | `clientId` |
| `INTUNE_CLIENT_SECRET` | `clientSecret` |
| `INTUNE_TENANT_ID` | `tenantId` |
| `INTUNE_POLL_INTERVAL` | `pollInterval` |
| `INTUNE_CRON_SCHEDULE` | `cronSchedule` |
| `INTUNE_DEVICE_OS_FILTER` | `deviceOsFilter` (comma-separated) |
| `INTUNE_ENABLE_PROMETHEUS` | `enablePrometheus` |
| `INTUNE_PROMETHEUS_PORT` | `prometheusPort` |
| `INTUNE_LOG_LEVEL` | `logLevel` |
| `INTUNE_DATABASE_BACKENDS` | `database.backends` (comma-separated) |
| `INTUNE_TABLE_NAME` | `database.tableName` |
| `INTUNE_SQLITE_PATH` | `database.sqlitePath` |
| `INTUNE_POSTGRES_CONNECTION` | `database.postgres.connectionString` |
| `INTUNE_MSSQL_CONNECTION` | `database.mssql.connectionString` |

### Environment Variable Examples

**Windows (PowerShell)**:
```powershell
$env:INTUNE_CLIENT_ID = "your-client-id"
$env:INTUNE_CLIENT_SECRET = "your-client-secret"
$env:INTUNE_TENANT_ID = "your-tenant-id"
$env:INTUNE_DEVICE_OS_FILTER = "Windows,macOS"
```

**Linux/macOS (Bash)**:
```bash
export INTUNE_CLIENT_ID="your-client-id"
export INTUNE_CLIENT_SECRET="your-client-secret"
export INTUNE_TENANT_ID="your-tenant-id"
export INTUNE_DEVICE_OS_FILTER="Windows,macOS"
```

**.env File**:
```env
INTUNE_CLIENT_ID=your-client-id
INTUNE_CLIENT_SECRET=your-client-secret
INTUNE_TENANT_ID=your-tenant-id
INTUNE_POLL_INTERVAL=30m
INTUNE_DEVICE_OS_FILTER=Windows,macOS,Android,iOS
INTUNE_ENABLE_PROMETHEUS=true
INTUNE_PROMETHEUS_PORT=9898
INTUNE_LOG_LEVEL=info
```

## Configuration Examples

### Minimal Configuration (SQLite)

```json
{
  "clientId": "12345678-1234-1234-1234-123456789012",
  "clientSecret": "your-secret-here",
  "tenantId": "87654321-4321-4321-4321-210987654321"
}
```

### Multi-Database Configuration

```json
{
  "clientId": "12345678-1234-1234-1234-123456789012",
  "clientSecret": "your-secret-here", 
  "tenantId": "87654321-4321-4321-4321-210987654321",
  "pollInterval": "30m",
  "deviceOsFilter": ["Windows", "macOS"],
  "database": {
    "backends": ["sqlite", "postgres"],
    "sqlitePath": "./data/devices.db",
    "postgres": {
      "connectionString": "postgres://intune_user:password@db.company.com:5432/intune_devices"
    }
  }
}
```

### Enterprise Configuration

```json
{
  "clientId": "12345678-1234-1234-1234-123456789012",
  "clientSecret": "your-secret-here",
  "tenantId": "87654321-4321-4321-4321-210987654321",
  "cronSchedule": "0 */6 * * *",
  "deviceOsFilter": ["Windows", "macOS", "Android", "iOS"],
  "enablePrometheus": true,
  "prometheusPort": 9898,
  "logLevel": "info",
  "database": {
    "backends": ["postgres", "mssql"],
    "tableName": "intune_devices",
    "postgres": {
      "connectionString": "postgres://intune_sync:secure_password@postgres.internal:5432/device_inventory?sslmode=require"
    },
    "mssql": {
      "connectionString": "server=sqlserver.internal;database=DeviceInventory;uid=intune_sync;pwd=secure_password;encrypt=true"
    }
  }
}
```

## Security Considerations

1. **Protect Secrets**:
   - Never commit `config.json` with real secrets to version control
   - Use environment variables for sensitive data in production
   - Restrict file permissions on configuration files

2. **Database Security**:
   - Use dedicated database users with minimal required permissions
   - Enable SSL/TLS for database connections
   - Regularly rotate database passwords

3. **Network Security**:
   - Restrict Prometheus metrics port access
   - Use firewalls to limit outbound connections
   - Monitor network traffic for anomalies

## Validation

The application validates configuration on startup and will report errors for:
- Missing required fields
- Invalid format values
- Unreachable database connections
- Invalid Azure credentials

Check the logs for detailed validation error messages.
