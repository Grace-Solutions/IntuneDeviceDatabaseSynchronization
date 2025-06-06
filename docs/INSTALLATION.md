# Installation Guide

> **📖 Service Management**: For detailed service management documentation including advanced configuration, troubleshooting, and platform-specific details, see [SERVICE_MANAGEMENT.md](SERVICE_MANAGEMENT.md).

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
   cd /opt/msgraph-db-synchronizer
   ```

2. **Install service** (automated):
   ```bash
   # This automatically creates service user, systemd service file, and enables the service
   sudo ./MSGraphDBSynchronizer install
   ```

3. **Start the service**:
   ```bash
   sudo ./MSGraphDBSynchronizer start
   ```

#### Manual systemd Setup (Alternative)

If you prefer manual setup or need customization:

1. **Create service user**:
   ```bash
   sudo useradd -r -s /bin/false msgraph-db-synchronizer
   sudo chown -R msgraph-db-synchronizer:msgraph-db-synchronizer /opt/msgraph-db-synchronizer
   ```

2. **Create systemd service file**:
   ```bash
   sudo tee /etc/systemd/system/msgraph-db-synchronizer.service > /dev/null <<EOF
   [Unit]
   Description=Microsoft Graph Database Synchronizer Service
   After=network.target
   Wants=network.target

   [Service]
   Type=simple
   User=msgraph-db-synchronizer
   Group=msgraph-db-synchronizer
   WorkingDirectory=/opt/msgraph-db-synchronizer
   ExecStart=/opt/msgraph-db-synchronizer/MSGraphDBSynchronizer run
   Restart=always
   RestartSec=10
   StandardOutput=journal
   StandardError=journal

   # Security settings
   NoNewPrivileges=true
   PrivateTmp=true
   ProtectSystem=strict
   ProtectHome=true
   ReadWritePaths=/opt/msgraph-db-synchronizer

   [Install]
   WantedBy=multi-user.target
   EOF
   ```

3. **Enable and start service**:
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

#### Using launchd (Recommended)

1. **Extract and install**:
   ```bash
   sudo unzip MSGraphDBSynchronizer-*.zip -d /opt/msgraph-db-synchronizer
   sudo chmod +x /opt/msgraph-db-synchronizer/MSGraphDBSynchronizer
   cd /opt/msgraph-db-synchronizer
   ```

2. **Install service** (automated):
   ```bash
   # This automatically creates service user, launchd plist, and loads the service
   sudo ./MSGraphDBSynchronizer install
   ```

3. **Start the service**:
   ```bash
   sudo ./MSGraphDBSynchronizer start
   ```

#### Manual launchd Setup (Alternative)

If you prefer manual setup or need customization:

1. **Create service user**:
   ```bash
   # Find available UID in system range
   for uid in {200..400}; do
       if ! dscl . list /Users UniqueID | grep -q " $uid$"; then
           echo "Using UID: $uid"
           break
       fi
   done

   # Create user
   sudo dscl . create /Users/_msgraphsync
   sudo dscl . create /Users/_msgraphsync UserShell /usr/bin/false
   sudo dscl . create /Users/_msgraphsync RealName "MSGraphDBSynchronizer Service User"
   sudo dscl . create /Users/_msgraphsync UniqueID $uid
   sudo dscl . create /Users/_msgraphsync PrimaryGroupID $uid
   sudo dscl . create /Users/_msgraphsync NFSHomeDirectory /var/empty
   ```

2. **Create launch daemon**:
   ```bash
   sudo tee /Library/LaunchDaemons/com.gracesolutions.msgraph-db-synchronizer.plist > /dev/null <<EOF
   <?xml version="1.0" encoding="UTF-8"?>
   <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
   <plist version="1.0">
   <dict>
       <key>Label</key>
       <string>com.gracesolutions.msgraph-db-synchronizer</string>
       <key>ProgramArguments</key>
       <array>
           <string>/opt/msgraph-db-synchronizer/MSGraphDBSynchronizer</string>
           <string>run</string>
       </array>
       <key>WorkingDirectory</key>
       <string>/opt/msgraph-db-synchronizer</string>
       <key>RunAtLoad</key>
       <true/>
       <key>KeepAlive</key>
       <true/>
       <key>StandardOutPath</key>
       <string>/var/log/msgraph-db-synchronizer.log</string>
       <key>StandardErrorPath</key>
       <string>/var/log/msgraph-db-synchronizer.error.log</string>
       <key>UserName</key>
       <string>_msgraphsync</string>
       <key>GroupName</key>
       <string>_msgraphsync</string>
   </dict>
   </plist>
   EOF
   ```

3. **Set permissions and load service**:
   ```bash
   sudo chown root:wheel /Library/LaunchDaemons/com.gracesolutions.msgraph-db-synchronizer.plist
   sudo chmod 644 /Library/LaunchDaemons/com.gracesolutions.msgraph-db-synchronizer.plist
   sudo launchctl load /Library/LaunchDaemons/com.gracesolutions.msgraph-db-synchronizer.plist
   ```

## Verification

1. **Check service status**:
   ```bash
   # All platforms (unified command)
   sudo ./MSGraphDBSynchronizer status

   # Platform-specific alternatives:
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
