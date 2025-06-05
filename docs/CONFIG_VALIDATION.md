# Configuration Validation

This document describes the comprehensive configuration validation system that helps ensure your Intune Device Database Synchronization service is properly configured.

## Overview

The configuration validator provides:
- **Syntax validation**: JSON structure and format checking
- **Semantic validation**: Business logic and value constraints
- **Security checks**: Identifies potential security issues
- **Best practice recommendations**: Performance and reliability suggestions
- **Detailed error reporting**: Precise error locations and suggestions

## Features

### 🔍 **Comprehensive Validation**
- **JSON syntax**: Validates JSON structure with line/column error reporting
- **Required fields**: Ensures all mandatory configuration is present
- **Data types**: Validates field types and formats
- **Value ranges**: Checks numeric ranges and constraints
- **Cross-field validation**: Validates relationships between settings

### 🛡️ **Security Validation**
- **Placeholder detection**: Identifies template values that need replacement
- **UUID format**: Validates Azure client/tenant ID formats
- **URL security**: Checks for HTTPS usage in webhooks
- **Connection string masking**: Safely displays connection strings in output

### 💡 **Smart Recommendations**
- **Performance warnings**: Identifies settings that may impact performance
- **Best practices**: Suggests optimal configuration values
- **Compatibility checks**: Warns about deprecated or problematic settings
- **Optimization suggestions**: Recommends improvements

## Usage

### Command Line Validation

Validate your configuration file:
```bash
# Validate default config.json
./IntuneDeviceDatabaseSynchronization validate

# Validate specific file
./IntuneDeviceDatabaseSynchronization validate --config my-config.json

# Validate and show detailed output
./IntuneDeviceDatabaseSynchronization validate --config config.json --verbose
```

### Exit Codes
- `0`: Configuration is valid
- `1`: Configuration has errors

## Validation Categories

### 1. **Errors** 🚨
Critical issues that prevent the service from running:

```
❌ Configuration validation failed!

🚨 Errors (3):
  1. [clientId] Client ID must be a valid UUID (current: 'invalid-uuid') 
     (expected: XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX) at line 2, column 15
  2. [database.backends] At least one database backend must be specified
  3. [prometheusPort] Port number must be between 1 and 65535 (current: '0')
```

### 2. **Warnings** ⚠️
Issues that may cause problems but don't prevent startup:

```
⚠️  Warnings (2):
  1. [webhook.url] Webhook URL should use HTTPS for security 
     (Recommendation: Use https:// instead of http://)
  2. [pollInterval] Very short poll interval may cause rate limiting 
     (Recommendation: Consider using at least 1 minute interval)
```

### 3. **Suggestions** 💡
Optimization and improvement recommendations:

```
💡 Suggestions (2):
  1. [logLevel] Debug/trace logging may impact performance in production 
     (suggested: 'info')
  2. [webhook.secret] Consider adding a webhook secret for authentication 
     (suggested: 'your-webhook-secret')
```

## Validation Rules

### Authentication Configuration

#### **Client ID**
- ✅ **Required**: Must be present
- ✅ **Format**: Must be valid UUID format
- ❌ **Placeholder**: Cannot contain "YOUR_" or "your-"

```json
{
  "clientId": "12345678-1234-1234-1234-123456789012"  // ✅ Valid
}
```

#### **Client Secret**
- ✅ **Required**: Must be present
- ⚠️ **Length**: Warning if less than 10 characters
- ❌ **Placeholder**: Cannot contain "YOUR_" or "your-"

#### **Tenant ID**
- ✅ **Required**: Must be present
- ✅ **Format**: Must be valid UUID format
- ❌ **Placeholder**: Cannot contain "YOUR_" or "your-"

### Sync Configuration

#### **Poll Interval**
- ✅ **Format**: Must be valid duration (e.g., "30s", "5m", "1h")
- ⚠️ **Range**: Warning if less than 1 minute or more than 24 hours
- 💡 **Conflict**: Note if both pollInterval and cronSchedule are set

```json
{
  "pollInterval": "1h",        // ✅ Valid
  "pollInterval": "30s",       // ⚠️ Warning: too short
  "pollInterval": "invalid"    // ❌ Error: invalid format
}
```

#### **Cron Schedule**
- ✅ **Format**: Must be valid cron expression (5 or 6 fields)
- 💡 **Precedence**: Note that cron takes precedence over pollInterval

#### **Device OS Filter**
- ✅ **Values**: Must be valid OS types (Windows, macOS, Android, iOS, Linux, *)
- 💡 **Empty**: Suggestion if no filter specified

### Database Configuration

#### **Backends**
- ✅ **Required**: At least one backend must be specified
- ✅ **Valid types**: Must be "sqlite", "postgres", or "mssql"

#### **SQLite Configuration**
- ✅ **Path required**: When using sqlite backend
- ⚠️ **Directory**: Warning if parent directory doesn't exist

#### **PostgreSQL Configuration**
- ✅ **Connection string**: Required when using postgres backend
- ✅ **Format**: Must start with "postgres://" or "postgresql://"
- 🛡️ **Security**: Passwords are masked in output

#### **MSSQL Configuration**
- ✅ **Connection string**: Required when using mssql backend
- ✅ **Format**: Must contain "server=" or "data source="
- 🛡️ **Security**: Passwords are masked in output

#### **Table Name**
- ✅ **Required**: Must be present
- ✅ **Format**: Must be valid SQL identifier (alphanumeric + underscore)

### Monitoring Configuration

#### **Prometheus Port**
- ❌ **Zero**: Cannot be 0
- ⚠️ **Privileged**: Warning if less than 1024
- ❌ **Range**: Must be 1-65535

#### **Log Level**
- ✅ **Valid values**: trace, debug, info, warn, error
- 💡 **Performance**: Suggestion for debug/trace in production

### Webhook Configuration

#### **URL Validation**
- ✅ **Required**: When webhooks enabled
- ✅ **Format**: Must be valid URL
- ⚠️ **Security**: Warning for HTTP (recommend HTTPS)

#### **Timeout and Retries**
- ❌ **Zero timeout**: Cannot be 0
- ⚠️ **Long timeout**: Warning if > 300 seconds
- ⚠️ **Many retries**: Warning if > 10 attempts

#### **Events**
- ⚠️ **Empty**: Warning if no events specified
- 💡 **Secret**: Suggestion to add webhook secret

### Rate Limiting Configuration

#### **Request Limits**
- ❌ **Zero requests**: Cannot be 0
- ⚠️ **High rate**: Warning if > 1000 requests/minute

#### **Retry Configuration**
- ❌ **Invalid multiplier**: Must be >= 1.0
- ⚠️ **High multiplier**: Warning if > 10.0
- ⚠️ **Long delays**: Warning if max delay > 1 hour

### Mock API Configuration

#### **Device Count**
- ⚠️ **High count**: Warning if > 10,000 devices

#### **Probabilities**
- ❌ **Invalid range**: Must be 0.0-1.0
- 💡 **Enabled**: Note when mock API is enabled

## Error Examples

### JSON Syntax Errors
```
❌ Configuration validation failed!

🚨 Errors (1):
  1. [root] JSON syntax error: expected `,` or `}` at line 5 column 3
```

### Missing Required Fields
```
🚨 Errors (2):
  1. [clientId] Client ID is required for Azure authentication
  2. [database.backends] At least one database backend must be specified
```

### Invalid Formats
```
🚨 Errors (3):
  1. [clientId] Client ID must be a valid UUID 
     (current: 'invalid-uuid') (expected: XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX)
  2. [pollInterval] Poll interval must be a valid duration 
     (current: 'invalid') (expected: Examples: '30s', '5m', '1h', '2h30m')
  3. [webhook.url] Invalid webhook URL format 
     (current: 'not-a-url') (expected: https://example.com/webhook)
```

### Placeholder Values
```
🚨 Errors (3):
  1. [clientId] Client ID appears to be a placeholder value 
     (current: 'YOUR_CLIENT_ID') (expected: Replace with actual Azure client ID)
  2. [clientSecret] Client secret appears to be a placeholder value 
     (expected: Replace with actual Azure client secret)
  3. [tenantId] Tenant ID appears to be a placeholder value 
     (current: 'YOUR_TENANT_ID') (expected: Replace with actual Azure tenant ID)
```

## Integration with CI/CD

### GitHub Actions
```yaml
name: Validate Configuration
on: [push, pull_request]

jobs:
  validate-config:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Build application
        run: cargo build --release
      - name: Validate configuration
        run: ./target/release/IntuneDeviceDatabaseSynchronization validate
```

### Pre-commit Hook
```bash
#!/bin/bash
# .git/hooks/pre-commit
./IntuneDeviceDatabaseSynchronization validate
if [ $? -ne 0 ]; then
    echo "Configuration validation failed. Please fix errors before committing."
    exit 1
fi
```

### Docker Health Check
```dockerfile
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD ./IntuneDeviceDatabaseSynchronization validate || exit 1
```

## Best Practices

### 1. **Regular Validation**
- Validate configuration after any changes
- Include validation in deployment pipelines
- Set up automated validation checks

### 2. **Security**
- Never commit real credentials to version control
- Use environment variables for sensitive values
- Regularly rotate secrets and update configuration

### 3. **Testing**
- Validate configuration in all environments
- Test with different configuration combinations
- Use mock API for configuration testing

### 4. **Documentation**
- Document configuration changes
- Keep configuration templates updated
- Share validation results with team

## Troubleshooting

### Common Issues

#### **UUID Format Errors**
```bash
# Generate valid UUIDs
uuidgen  # On macOS/Linux
# Or use online UUID generator
```

#### **Duration Format Errors**
```json
{
  "pollInterval": "1h30m",    // ✅ Valid
  "pollInterval": "90m",      // ✅ Valid  
  "pollInterval": "5400s",    // ✅ Valid
  "pollInterval": "1.5h"      // ❌ Invalid
}
```

#### **Connection String Issues**
```json
{
  // PostgreSQL
  "connectionString": "postgres://user:pass@host:5432/db",     // ✅ Valid
  "connectionString": "postgresql://user:pass@host:5432/db",   // ✅ Valid
  
  // MSSQL
  "connectionString": "server=host;database=db;uid=user;pwd=pass",  // ✅ Valid
  "connectionString": "Data Source=host;Initial Catalog=db;..."     // ✅ Valid
}
```

### Getting Help

If validation fails with unclear errors:
1. Check the error message for specific field paths
2. Refer to the configuration examples in this documentation
3. Use the mock API configuration for testing
4. Check the logs for additional context
