# Troubleshooting Guide

## Common Issues

### Authentication Problems

#### Error: "Authentication failed"
**Symptoms**: Service fails to start with authentication errors in logs.

**Solutions**:
1. **Verify Azure App Registration**:
   - Check Client ID, Client Secret, and Tenant ID are correct
   - Ensure the client secret hasn't expired
   - Verify API permissions are granted and admin consent given

2. **Check Required Permissions**:
   ```
   Microsoft Graph Application Permissions:
   - DeviceManagementManagedDevices.Read.All
   - Device.Read.All (optional)
   ```

3. **Test Authentication**:
   ```bash
   # Run in debug mode to see detailed auth logs
   RUST_LOG=debug ./IntuneDeviceDatabaseSynchronization run
   ```

#### Error: "Token refresh failed"
**Cause**: Network connectivity or Azure service issues.

**Solutions**:
- Check internet connectivity to `login.microsoftonline.com`
- Verify firewall allows outbound HTTPS (port 443)
- Check Azure service status

### Database Connection Issues

#### Error: "Failed to connect to database"
**Symptoms**: Service starts but fails during database operations.

**PostgreSQL Solutions**:
1. **Check Connection String Format**:
   ```
   postgres://username:password@hostname:port/database_name
   postgresql://username:password@hostname:port/database_name?sslmode=require
   ```

2. **Verify Database Exists**:
   ```sql
   CREATE DATABASE intune_devices;
   ```

3. **Check User Permissions**:
   ```sql
   GRANT ALL PRIVILEGES ON DATABASE intune_devices TO username;
   ```

**MSSQL Solutions**:
1. **Check Connection String Format**:
   ```
   server=hostname;database=dbname;uid=username;pwd=password
   server=hostname;database=dbname;trusted_connection=true
   ```

2. **Enable TCP/IP Protocol**:
   - SQL Server Configuration Manager → Protocols → TCP/IP → Enabled

3. **Check SQL Server Authentication**:
   - Mixed mode authentication must be enabled for SQL auth

**SQLite Solutions**:
1. **Check File Permissions**:
   ```bash
   # Ensure directory is writable
   mkdir -p ./output
   chmod 755 ./output
   ```

2. **Verify Disk Space**:
   ```bash
   df -h ./output
   ```

### Network Connectivity Issues

#### Error: "Failed to fetch devices from Microsoft Graph"
**Symptoms**: Authentication succeeds but device fetching fails.

**Solutions**:
1. **Check Microsoft Graph Connectivity**:
   ```bash
   curl -v https://graph.microsoft.com/v1.0/deviceManagement/managedDevices
   ```

2. **Verify DNS Resolution**:
   ```bash
   nslookup graph.microsoft.com
   ```

3. **Check Proxy Settings**:
   ```bash
   # Set proxy if required
   export HTTPS_PROXY=http://proxy.company.com:8080
   export HTTP_PROXY=http://proxy.company.com:8080
   ```

4. **Firewall Configuration**:
   - Allow outbound HTTPS (443) to:
     - `graph.microsoft.com`
     - `login.microsoftonline.com`

### Service Management Issues

#### Windows Service Won't Start
**Symptoms**: Service fails to start or stops immediately.

**Solutions**:
1. **Check Event Logs**:
   ```powershell
   Get-EventLog -LogName Application -Source "IntuneDeviceDatabaseSynchronization" -Newest 10
   ```

2. **Run in Console Mode**:
   ```powershell
   # Test configuration by running in foreground
   .\IntuneDeviceDatabaseSynchronization.exe run
   ```

3. **Check Service Account Permissions**:
   - Ensure service account has read access to config files
   - Verify write access to log directory
   - Check database connection permissions

#### Linux systemd Service Issues
**Symptoms**: Service fails to start or crashes.

**Solutions**:
1. **Check Service Status**:
   ```bash
   sudo systemctl status intune-device-sync
   sudo journalctl -u intune-device-sync -f
   ```

2. **Verify File Permissions**:
   ```bash
   sudo chown -R intune-sync:intune-sync /opt/intune-device-sync
   sudo chmod +x /opt/intune-device-sync/IntuneDeviceDatabaseSynchronization
   ```

3. **Check SELinux (if enabled)**:
   ```bash
   sudo setsebool -P httpd_can_network_connect 1
   sudo restorecon -R /opt/intune-device-sync
   ```

### Configuration Issues

#### Error: "Invalid configuration"
**Symptoms**: Service fails to start with configuration validation errors.

**Solutions**:
1. **Validate JSON Syntax**:
   ```bash
   # Check for JSON syntax errors
   python -m json.tool config.json
   ```

2. **Check Required Fields**:
   ```json
   {
     "clientId": "required",
     "clientSecret": "required", 
     "tenantId": "required"
   }
   ```

3. **Environment Variable Override**:
   ```bash
   # Use environment variables to override config
   export INTUNE_CLIENT_ID="your-client-id"
   export INTUNE_CLIENT_SECRET="your-client-secret"
   export INTUNE_TENANT_ID="your-tenant-id"
   ```

### Performance Issues

#### High Memory Usage
**Symptoms**: Service consumes excessive memory over time.

**Solutions**:
1. **Adjust Sync Interval**:
   ```json
   {
     "pollInterval": "2h"  // Increase interval
   }
   ```

2. **Enable Device Filtering**:
   ```json
   {
     "deviceOsFilter": ["Windows"]  // Reduce device count
   }
   ```

3. **Monitor with Metrics**:
   ```bash
   curl http://localhost:9898/metrics | grep memory
   ```

#### Slow Database Operations
**Symptoms**: Long sync times, database timeouts.

**Solutions**:
1. **Check Database Indexes**:
   ```sql
   -- PostgreSQL
   CREATE INDEX CONCURRENTLY idx_devices_uuid ON devices(uuid);
   CREATE INDEX CONCURRENTLY idx_devices_updated_at ON devices(updated_at);
   ```

2. **Optimize Connection Pool**:
   ```json
   {
     "database": {
       "postgres": {
         "connectionString": "postgres://user:pass@host/db?max_connections=10"
       }
     }
   }
   ```

3. **Monitor Query Performance**:
   ```sql
   -- PostgreSQL
   SELECT query, mean_time, calls FROM pg_stat_statements 
   WHERE query LIKE '%devices%' ORDER BY mean_time DESC;
   ```

### Logging and Debugging

#### Enable Debug Logging
```bash
# Environment variable
export RUST_LOG=debug

# Or in config.json
{
  "logLevel": "debug"
}
```

#### Log File Locations
- **Windows**: `logs\app.log`
- **Linux**: `/opt/intune-device-sync/logs/app.log`
- **macOS**: `/opt/intune-device-sync/logs/app.log`

#### Log Analysis Commands
```bash
# View recent logs
tail -f logs/app.log

# Search for errors
grep -i error logs/app.log

# Filter by component
grep "\[Sync\]" logs/app.log

# Count sync operations
grep "sync operation" logs/app.log | wc -l
```

### Metrics and Monitoring

#### Prometheus Metrics Not Available
**Symptoms**: Cannot access `http://localhost:9898/metrics`

**Solutions**:
1. **Check Configuration**:
   ```json
   {
     "enablePrometheus": true,
     "prometheusPort": 9898
   }
   ```

2. **Verify Port Binding**:
   ```bash
   netstat -tlnp | grep 9898
   ```

3. **Check Firewall**:
   ```bash
   # Linux
   sudo ufw allow 9898
   
   # Windows
   netsh advfirewall firewall add rule name="Intune Metrics" dir=in action=allow protocol=TCP localport=9898
   ```

### Build and Deployment Issues

#### Cross-Platform Build Failures
**Symptoms**: Build fails for non-native targets.

**Solutions**:
1. **Install Required Toolchains**:
   ```bash
   # For Linux target (from Windows/macOS)
   rustup target add x86_64-unknown-linux-gnu
   
   # For macOS target (requires macOS or cross-compilation tools)
   rustup target add x86_64-apple-darwin
   ```

2. **Use Platform-Specific Builds**:
   - Build on the target platform for best compatibility
   - Use CI/CD with multiple runners

3. **Docker for Linux Builds**:
   ```bash
   docker build --target builder -t intune-builder .
   docker run --rm -v $(pwd):/workspace intune-builder cargo build --release
   ```

## Advanced Error Handling Examples

### Retry Logic Implementation

The service implements sophisticated retry logic for transient failures:

```rust
// Example: HTTP request with exponential backoff
async fn fetch_with_retry<T>(
    operation: impl Fn() -> Result<T>,
    max_attempts: u32,
    base_delay: Duration,
) -> Result<T> {
    let mut attempt = 1;
    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt >= max_attempts => return Err(e),
            Err(e) if is_retryable_error(&e) => {
                let delay = base_delay * 2_u32.pow(attempt - 1);
                warn!("Attempt {} failed, retrying in {:?}: {}", attempt, delay, e);
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
            Err(e) => return Err(e), // Non-retryable error
        }
    }
}
```

### Circuit Breaker Pattern

For external service failures:

```rust
pub struct CircuitBreaker {
    failure_count: AtomicU32,
    last_failure: AtomicU64,
    failure_threshold: u32,
    recovery_timeout: Duration,
    state: AtomicU8, // 0=Closed, 1=Open, 2=HalfOpen
}

impl CircuitBreaker {
    pub async fn call<T>(&self, operation: impl Future<Output = Result<T>>) -> Result<T> {
        match self.state() {
            CircuitState::Open => {
                if self.should_attempt_reset() {
                    self.set_state(CircuitState::HalfOpen);
                } else {
                    return Err(anyhow::anyhow!("Circuit breaker is open"));
                }
            }
            CircuitState::HalfOpen => {
                // Allow one test request
            }
            CircuitState::Closed => {
                // Normal operation
            }
        }

        match operation.await {
            Ok(result) => {
                self.on_success();
                Ok(result)
            }
            Err(e) => {
                self.on_failure();
                Err(e)
            }
        }
    }
}
```

### Database Connection Recovery

Automatic database reconnection:

```rust
pub struct DatabaseManager {
    pool: Arc<Mutex<Option<Pool>>>,
    config: DatabaseConfig,
}

impl DatabaseManager {
    pub async fn execute_with_retry<T>(&self, operation: impl Fn(&Pool) -> Result<T>) -> Result<T> {
        const MAX_RETRIES: u32 = 3;

        for attempt in 1..=MAX_RETRIES {
            let pool = self.get_or_create_pool().await?;

            match operation(&pool) {
                Ok(result) => return Ok(result),
                Err(e) if self.is_connection_error(&e) && attempt < MAX_RETRIES => {
                    warn!("Database connection error on attempt {}, reconnecting: {}", attempt, e);
                    self.invalidate_pool().await;
                    tokio::time::sleep(Duration::from_secs(attempt as u64)).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(anyhow::anyhow!("Database operation failed after {} attempts", MAX_RETRIES))
    }
}
```

### Graceful Degradation

When external services are unavailable:

```rust
pub struct SyncService {
    primary_source: GraphApiClient,
    fallback_enabled: bool,
    last_successful_sync: Arc<Mutex<Option<DateTime<Utc>>>>,
}

impl SyncService {
    pub async fn sync_devices(&self) -> Result<SyncResult> {
        match self.primary_source.fetch_devices().await {
            Ok(devices) => {
                self.update_last_sync_time().await;
                self.process_devices(devices).await
            }
            Err(e) if self.fallback_enabled => {
                warn!("Primary sync failed, using fallback strategy: {}", e);
                self.fallback_sync().await
            }
            Err(e) => {
                error!("Sync failed and no fallback available: {}", e);
                Err(e)
            }
        }
    }

    async fn fallback_sync(&self) -> Result<SyncResult> {
        // Use cached data or alternative data source
        if let Some(last_sync) = self.get_last_sync_time().await {
            if last_sync.elapsed() < Duration::from_hours(24) {
                info!("Using cached device data from last successful sync");
                return self.use_cached_data().await;
            }
        }

        Err(anyhow::anyhow!("No recent cached data available for fallback"))
    }
}
```

## Getting Help

### Log Collection
When reporting issues, include:

1. **Application Logs**:
   ```bash
   # Last 100 lines with timestamps
   tail -100 logs/app.log
   ```

2. **System Information**:
   ```bash
   # Version info
   ./IntuneDeviceDatabaseSynchronization version
   
   # System details
   uname -a  # Linux/macOS
   systeminfo  # Windows
   ```

3. **Configuration** (sanitized):
   ```json
   {
     "clientId": "12345678-****-****-****-************",
     "tenantId": "87654321-****-****-****-************",
     "pollInterval": "1h",
     "deviceOsFilter": ["Windows", "macOS"]
   }
   ```

### Support Channels
- GitHub Issues: Report bugs and feature requests
- Documentation: Check docs folder for detailed guides
- Metrics: Use Prometheus metrics for operational insights
