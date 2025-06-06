# Service Management

MSGraphDBSynchronizer provides comprehensive service management capabilities across Windows, Linux, and macOS platforms. The application can be installed and managed as a native system service on all supported operating systems.

## Overview

The service management system provides:
- **Cross-platform compatibility**: Native service integration for Windows, Linux (systemd), and macOS (launchd)
- **Automated installation**: One-command service installation with proper user creation and permissions
- **Security-focused**: Runs with minimal privileges using dedicated service accounts
- **Enterprise-ready**: Proper logging, monitoring, and management integration

## Commands

All service management commands require elevated privileges (Administrator on Windows, root on Linux/macOS).

### Install Service
```bash
# Install the service on the current platform
sudo ./MSGraphDBSynchronizer install
```

### Uninstall Service
```bash
# Remove the service from the current platform
# This automatically stops the service before removing it
sudo ./MSGraphDBSynchronizer uninstall
```

### Start Service
```bash
# Start the service
sudo ./MSGraphDBSynchronizer start
```

### Stop Service
```bash
# Stop the service
sudo ./MSGraphDBSynchronizer stop
```

### Restart Service
```bash
# Restart the service (stop + start with 2-second delay)
sudo ./MSGraphDBSynchronizer restart
```

### Check Status
```bash
# Show service status
sudo ./MSGraphDBSynchronizer status
```

## Platform-Specific Implementation

### Windows (Windows Service)

**Service Details:**
- **Service Name**: `MSGraphDBSynchronizer`
- **Display Name**: `MSGraphDBSynchronizer Service`
- **Start Type**: Automatic
- **Account**: Local System (can be changed post-installation)

**Installation Process:**
1. Creates Windows service entry in Service Control Manager
2. Configures automatic startup
3. Sets proper service permissions

**Uninstallation Process:**
1. Stops the service if it's running
2. Waits for graceful shutdown (2 seconds)
3. Removes the service from Service Control Manager

**Management:**
```powershell
# Alternative Windows-specific commands
sc start MSGraphDBSynchronizer
sc stop MSGraphDBSynchronizer
sc query MSGraphDBSynchronizer
```

### Linux (systemd)

**Service Details:**
- **Service Name**: `msgraph-db-synchronizer`
- **Service File**: `/etc/systemd/system/msgraph-db-synchronizer.service`
- **User Account**: `msgraph-db-synchronizer` (created automatically)
- **Working Directory**: Application installation directory

**Installation Process:**
1. Creates dedicated system user (`msgraph-db-synchronizer`)
2. Generates systemd service file with security hardening
3. Enables service for automatic startup
4. Configures proper permissions and isolation

**Uninstallation Process:**
1. Stops the service if it's running
2. Disables the service from automatic startup
3. Removes the systemd service file
4. Reloads systemd daemon configuration

**Service File Features:**
- **Security Hardening**: `NoNewPrivileges`, `PrivateTmp`, `ProtectSystem=strict`
- **Automatic Restart**: Service restarts on failure with 10-second delay
- **Logging**: Integrated with systemd journal
- **Network Dependencies**: Waits for network availability

**Management:**
```bash
# Alternative systemd commands
sudo systemctl start msgraph-db-synchronizer
sudo systemctl stop msgraph-db-synchronizer
sudo systemctl status msgraph-db-synchronizer
sudo systemctl enable msgraph-db-synchronizer
sudo systemctl disable msgraph-db-synchronizer

# View logs
sudo journalctl -u msgraph-db-synchronizer -f
```

### macOS (launchd)

**Service Details:**
- **Service Name**: `com.gracesolutions.msgraph-db-synchronizer`
- **Plist File**: `/Library/LaunchDaemons/com.gracesolutions.msgraph-db-synchronizer.plist`
- **User Account**: `_msgraphsync` (created automatically)
- **Log Files**: `/var/log/msgraph-db-synchronizer.log` and `/var/log/msgraph-db-synchronizer.error.log`

**Installation Process:**
1. Creates dedicated system user (`_msgraphsync`) with UID in system range (200-400)
2. Generates launchd plist file with proper configuration
3. Sets correct ownership and permissions (root:wheel, 644)
4. Loads service for immediate and automatic startup

**Uninstallation Process:**
1. Stops the service if it's running
2. Unloads the service from launchd
3. Removes the plist file from LaunchDaemons

**Plist Features:**
- **Automatic Startup**: `RunAtLoad` and `KeepAlive` enabled
- **Logging**: Separate stdout and stderr log files
- **Working Directory**: Application installation directory
- **User Isolation**: Runs under dedicated service account

**Management:**
```bash
# Alternative launchctl commands
sudo launchctl load /Library/LaunchDaemons/com.gracesolutions.msgraph-db-synchronizer.plist
sudo launchctl unload /Library/LaunchDaemons/com.gracesolutions.msgraph-db-synchronizer.plist
sudo launchctl start com.gracesolutions.msgraph-db-synchronizer
sudo launchctl stop com.gracesolutions.msgraph-db-synchronizer
sudo launchctl list | grep msgraph

# View logs
tail -f /var/log/msgraph-db-synchronizer.log
tail -f /var/log/msgraph-db-synchronizer.error.log
```

## Security Considerations

### User Accounts

**Linux:**
- Creates system user `msgraph-db-synchronizer`
- No home directory, shell set to `/bin/false`
- Minimal system privileges

**macOS:**
- Creates system user `_msgraphsync`
- UID assigned in system range (200-400)
- No home directory, shell set to `/usr/bin/false`
- Minimal system privileges

**Windows:**
- Runs under Local System by default
- Can be reconfigured to use a dedicated service account post-installation

### Permissions

All platforms implement the principle of least privilege:
- **Read/Write Access**: Only to application directory and configured data paths
- **Network Access**: Required for Microsoft Graph API communication
- **System Access**: Minimal required permissions only

### File Permissions

**Linux/macOS:**
- Service files owned by root with appropriate permissions
- Application files accessible by service user
- Log files writable by service user

## Troubleshooting

### Common Issues

**Permission Denied:**
```bash
# Ensure running with elevated privileges
sudo ./MSGraphDBSynchronizer install
```

**Service Won't Start:**
```bash
# Check service status and logs
sudo ./MSGraphDBSynchronizer status

# Platform-specific log checking
# Linux:
sudo journalctl -u msgraph-db-synchronizer -n 50

# macOS:
tail -n 50 /var/log/msgraph-db-synchronizer.error.log

# Windows:
# Check Windows Event Viewer > Windows Logs > Application
```

**Configuration Issues:**
```bash
# Validate configuration before installing service
./MSGraphDBSynchronizer validate

# Check configuration file permissions
ls -la config.json
```

### Service User Issues

**Linux - User Creation Failed:**
```bash
# Check if user already exists
id msgraph-db-synchronizer

# Manually create if needed
sudo useradd --system --no-create-home --shell /bin/false msgraph-db-synchronizer
```

**macOS - UID Conflicts:**
```bash
# Check existing UIDs
dscl . list /Users UniqueID | sort -n -k2

# Find available UID in system range
for uid in {200..400}; do
    if ! dscl . list /Users UniqueID | grep -q " $uid$"; then
        echo "Available UID: $uid"
        break
    fi
done
```

## Best Practices

1. **Always validate configuration** before installing the service
2. **Use dedicated service accounts** (automatically created)
3. **Monitor service logs** regularly
4. **Test service restart** after configuration changes
5. **Keep service files secure** with proper permissions
6. **Use system package managers** when available for updates

## Integration with Monitoring

The service integrates seamlessly with system monitoring:
- **Linux**: systemd status and journal integration
- **macOS**: launchd status and log file monitoring
- **Windows**: Windows Service Manager and Event Log integration

All platforms support:
- Service health monitoring
- Automatic restart on failure
- Log aggregation and rotation
- Performance metrics collection (when Prometheus is enabled)
