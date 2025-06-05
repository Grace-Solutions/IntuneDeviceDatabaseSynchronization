# Installation Guide

## Prerequisites

- **Operating System**: Windows 10/11, Linux (Ubuntu 20.04+), or macOS 10.15+
- **Microsoft Azure**: App Registration with appropriate permissions
- **Database** (Optional): PostgreSQL or MSSQL Server if using external databases
- **Network**: Internet access for Microsoft Graph API calls

## Azure App Registration Setup

1. **Create App Registration**:
   - Go to Azure Portal → Azure Active Directory → App registrations
   - Click "New registration"
   - Name: `IntuneDeviceDatabaseSynchronization`
   - Supported account types: "Accounts in this organizational directory only"
   - Click "Register"

2. **Configure API Permissions**:
   - Go to "API permissions"
   - Click "Add a permission" → Microsoft Graph → Application permissions
   - Add the following permissions:
     - `Device.Read.All`
     - `DeviceManagementManagedDevices.Read.All`
   - Click "Grant admin consent"

3. **Create Client Secret**:
   - Go to "Certificates & secrets"
   - Click "New client secret"
   - Description: `IntuneDeviceSync`
   - Expires: Choose appropriate duration
   - Copy the secret value (you won't see it again)

4. **Note the following values**:
   - Application (client) ID
   - Directory (tenant) ID
   - Client secret value

## Installation Methods

### Method 1: Download Pre-built Binaries (Recommended)

1. **Download the latest release**:
   - Go to the [Releases page](https://github.com/Grace-Solutions/MSGraphDBSynchronizer/releases)
   - Download the appropriate package for your platform:
     - Windows: `MSGraphDBSynchronizer-VERSION-windows-Release.zip`
     - Linux: `MSGraphDBSynchronizer-VERSION-linux-Release.zip`
     - macOS: `MSGraphDBSynchronizer-VERSION-macos-Release.zip`

2. **Extract the package**:
   ```bash
   # Windows (PowerShell)
   Expand-Archive -Path "MSGraphDBSynchronizer-*.zip" -DestinationPath "C:\MSGraphDBSynchronizer"

   # Linux/macOS
   unzip MSGraphDBSynchronizer-*.zip -d /opt/msgraph-db-synchronizer
   ```

3. **Configure the application**:
   - Edit `config.json` with your Azure app registration details
   - See [Configuration Guide](CONFIGURATION.md) for detailed setup

### Method 2: Build from Source

See [Build Guide](BUILD.md) for detailed build instructions.

## Platform-Specific Installation

### Windows

#### As a Windows Service (Recommended)

1. **Extract to Program Files**:
   ```powershell
   # Run as Administrator
   Expand-Archive -Path "MSGraphDBSynchronizer-*.zip" -DestinationPath "C:\Program Files\MSGraphDBSynchronizer"
   cd "C:\Program Files\MSGraphDBSynchronizer"
   ```

2. **Install the service**:
   ```powershell
   # Run as Administrator
   .\MSGraphDBSynchronizer.exe install
   ```

3. **Start the service**:
   ```powershell
   .\MSGraphDBSynchronizer.exe start
   ```

#### As a Console Application

1. **Extract to desired location**:
   ```powershell
   Expand-Archive -Path "MSGraphDBSynchronizer-*.zip" -DestinationPath "C:\MSGraphDBSynchronizer"
   cd "C:\MSGraphDBSynchronizer"
   ```

2. **Run in foreground**:
   ```powershell
   .\MSGraphDBSynchronizer.exe run
   ```

### Linux

#### Using systemd (Recommended)

1. **Extract and install**:
   ```bash
   sudo unzip MSGraphDBSynchronizer-*.zip -d /opt/msgraph-db-synchronizer
   sudo chmod +x /opt/msgraph-db-synchronizer/MSGraphDBSynchronizer
   ```

2. **Create systemd service**:
   ```bash
   sudo tee /etc/systemd/system/msgraph-db-synchronizer.service > /dev/null <<EOF
   [Unit]
   Description=Microsoft Graph Database Synchronizer Service
   After=network.target

   [Service]
   Type=simple
   User=msgraph-sync
   Group=msgraph-sync
   WorkingDirectory=/opt/msgraph-db-synchronizer
   ExecStart=/opt/msgraph-db-synchronizer/MSGraphDBSynchronizer run
   Restart=always
   RestartSec=10

   [Install]
   WantedBy=multi-user.target
   EOF
   ```

3. **Create service user**:
   ```bash
   sudo useradd -r -s /bin/false msgraph-sync
   sudo chown -R msgraph-sync:msgraph-sync /opt/msgraph-db-synchronizer
   ```

4. **Enable and start service**:
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable msgraph-db-synchronizer
   sudo systemctl start msgraph-db-synchronizer
   ```

#### Manual Execution

```bash
cd /opt/msgraph-db-synchronizer
./MSGraphDBSynchronizer run
```

### macOS

#### Using launchd

1. **Extract and install**:
   ```bash
   sudo unzip MSGraphDBSynchronizer-*.zip -d /opt/msgraph-db-synchronizer
   sudo chmod +x /opt/msgraph-db-synchronizer/MSGraphDBSynchronizer
   ```

2. **Create launch daemon**:
   ```bash
   sudo tee /Library/LaunchDaemons/com.yourorg.intune-device-sync.plist > /dev/null <<EOF
   <?xml version="1.0" encoding="UTF-8"?>
   <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
   <plist version="1.0">
   <dict>
       <key>Label</key>
       <string>com.yourorg.intune-device-sync</string>
       <key>ProgramArguments</key>
       <array>
           <string>/opt/intune-device-sync/IntuneDeviceDatabaseSynchronization</string>
           <string>run</string>
       </array>
       <key>WorkingDirectory</key>
       <string>/opt/intune-device-sync</string>
       <key>RunAtLoad</key>
       <true/>
       <key>KeepAlive</key>
       <true/>
   </dict>
   </plist>
   EOF
   ```

3. **Load and start service**:
   ```bash
   sudo launchctl load /Library/LaunchDaemons/com.yourorg.intune-device-sync.plist
   sudo launchctl start com.yourorg.intune-device-sync
   ```

## Verification

1. **Check service status**:
   ```bash
   # Windows
   MSGraphDBSynchronizer.exe status

   # Linux
   sudo systemctl status msgraph-db-synchronizer

   # macOS
   sudo launchctl list | grep msgraph-db-synchronizer
   ```

2. **Check logs**:
   - Logs are written to the `logs/` directory
   - Check `logs/app.log` for application logs

3. **Check metrics** (if enabled):
   - Open browser to `http://localhost:9898/metrics`
   - Should show Prometheus metrics

## Troubleshooting

### Common Issues

1. **Permission Errors**:
   - Ensure the service user has read/write access to the installation directory
   - Check database connection permissions

2. **Network Connectivity**:
   - Verify internet access to `graph.microsoft.com`
   - Check firewall settings for outbound HTTPS (443)

3. **Authentication Errors**:
   - Verify Azure app registration configuration
   - Check client ID, tenant ID, and client secret
   - Ensure API permissions are granted

4. **Database Connection Issues**:
   - Verify database server is accessible
   - Check connection string format
   - Ensure database exists (will be created automatically if possible)

### Log Analysis

Check the application logs for detailed error information:

```bash
# View recent logs
tail -f logs/app.log

# Search for errors
grep -i error logs/app.log
```

For more detailed troubleshooting, see [Troubleshooting Guide](TROUBLESHOOTING.md).
