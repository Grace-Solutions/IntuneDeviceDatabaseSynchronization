# Multi-Endpoint Configuration Guide

This guide covers the multi-endpoint feature that allows you to sync multiple Microsoft Graph API endpoints to separate database tables.

## Overview

The endpoints feature enables you to:
- Sync multiple Microsoft Graph API endpoints simultaneously
- Store data from different endpoints in separate database tables
- Configure custom field mappings and filters per endpoint
- Enable/disable specific endpoints independently
- Use predefined endpoint configurations or create custom ones

## Configuration Structure

The endpoints configuration is defined in the `endpoints` section of your config.json:

```json
{
  "endpoints": {
    "endpoints": [
      {
        "name": "devices",
        "endpointUrl": "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices",
        "tableName": "devices",
        "enabled": true,
        "queryParams": {},
        "selectFields": null,
        "filter": null,
        "fieldMappings": {}
      }
    ]
  }
}
```

## Endpoint Configuration Options

### Required Fields

- **name**: Unique identifier for the endpoint
- **endpointUrl**: Microsoft Graph API endpoint URL
- **tableName**: Database table name for storing data
- **enabled**: Whether this endpoint should be synchronized

### Optional Fields

- **syncInterval**: Override global sync interval for this endpoint
- **queryParams**: Additional query parameters for the API request
- **selectFields**: Array of fields to select from the API response
- **filter**: OData filter expression for the API query
- **fieldMappings**: Map source fields to different target field names

## Predefined Endpoints

### Devices (Default)
```json
{
  "name": "devices",
  "endpointUrl": "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices",
  "tableName": "devices",
  "enabled": true
}
```

### Users
```json
{
  "name": "users",
  "endpointUrl": "https://graph.microsoft.com/v1.0/users",
  "tableName": "users",
  "enabled": false,
  "selectFields": [
    "id",
    "userPrincipalName",
    "displayName",
    "mail",
    "jobTitle",
    "department",
    "companyName",
    "accountEnabled",
    "createdDateTime",
    "lastSignInDateTime"
  ]
}
```

### Groups
```json
{
  "name": "groups",
  "endpointUrl": "https://graph.microsoft.com/v1.0/groups",
  "tableName": "groups",
  "enabled": false,
  "selectFields": [
    "id",
    "displayName",
    "description",
    "groupTypes",
    "mail",
    "mailEnabled",
    "securityEnabled",
    "createdDateTime"
  ]
}
```

### Compliance Policies
```json
{
  "name": "compliance_policies",
  "endpointUrl": "https://graph.microsoft.com/v1.0/deviceManagement/deviceCompliancePolicies",
  "tableName": "compliance_policies",
  "enabled": false
}
```

## Advanced Configuration Examples

### Custom Field Selection
```json
{
  "name": "users_basic",
  "endpointUrl": "https://graph.microsoft.com/v1.0/users",
  "tableName": "users_basic",
  "enabled": true,
  "selectFields": ["id", "userPrincipalName", "displayName", "mail"],
  "filter": "accountEnabled eq true"
}
```

### Field Mappings
```json
{
  "name": "devices_mapped",
  "endpointUrl": "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices",
  "tableName": "devices_mapped",
  "enabled": true,
  "fieldMappings": {
    "deviceName": "device_display_name",
    "operatingSystem": "os_type",
    "serialNumber": "serial_num"
  }
}
```

### Custom Query Parameters
```json
{
  "name": "recent_devices",
  "endpointUrl": "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices",
  "tableName": "recent_devices",
  "enabled": true,
  "queryParams": {
    "$top": "100",
    "$orderby": "enrolledDateTime desc"
  },
  "filter": "enrolledDateTime ge 2024-01-01T00:00:00Z"
}
```

## Database Schema

Each endpoint automatically creates its own table with a dynamic schema based on the data received. Common fields added to all tables:

- **id**: Primary key (auto-generated if not present in source data)
- **last_sync_date_time**: Timestamp of last synchronization

### Table Creation

Tables are created automatically with the following approach:
- All fields are stored as text/varchar for maximum compatibility
- Complex objects (arrays, nested objects) are stored as JSON strings
- Primary key is based on the 'id' field from the source data
- If no 'id' field exists, a UUID is generated

## Permissions Required

Ensure your Azure App Registration has the appropriate permissions for each endpoint:

### Devices
- `DeviceManagementManagedDevices.Read.All`

### Users
- `User.Read.All`

### Groups
- `Group.Read.All`

### Compliance Policies
- `DeviceManagementConfiguration.Read.All`

## Monitoring and Metrics

Each endpoint is monitored separately with Prometheus metrics:
- Sync success/failure rates per endpoint
- Record counts per endpoint
- Sync duration per endpoint
- API rate limiting per endpoint

## Troubleshooting

### Common Issues

1. **Permission Errors**: Ensure your Azure app has the required permissions for each endpoint
2. **Table Creation Failures**: Check database permissions and schema validation
3. **Field Mapping Errors**: Verify source field names exist in the API response
4. **Rate Limiting**: Each endpoint shares the same rate limiter, so multiple endpoints may trigger limits faster

### Validation

Use the configuration validation command to check your endpoints configuration:

```bash
MSGraphDBSynchronizer validate --config config.json
```

### Logs

Enable debug logging to see detailed endpoint processing:

```bash
RUST_LOG=debug MSGraphDBSynchronizer run
```

## Migration from Single Endpoint

If you're upgrading from a version that only supported devices:

1. Your existing configuration will continue to work
2. If no `endpoints` section is present, a default devices endpoint is created
3. Your existing devices table and data remain unchanged
4. Add new endpoints gradually and test each one

## Best Practices

1. **Start Small**: Enable one endpoint at a time to test configuration
2. **Use Filters**: Apply OData filters to reduce data volume and API calls
3. **Select Fields**: Use `selectFields` to only retrieve needed data
4. **Monitor Resources**: Watch database size and API rate limits
5. **Test Permissions**: Validate Azure app permissions before enabling endpoints
6. **Backup Data**: Ensure backups are configured before adding new endpoints

## Example Complete Configuration

```json
{
  "clientId": "your-client-id",
  "clientSecret": "your-client-secret",
  "tenantId": "your-tenant-id",
  "pollInterval": "1h",
  "database": {
    "backends": ["sqlite"],
    "sqlitePath": "./data/msgraph_data.db"
  },
  "endpoints": {
    "endpoints": [
      {
        "name": "devices",
        "endpointUrl": "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices",
        "tableName": "devices",
        "enabled": true
      },
      {
        "name": "active_users",
        "endpointUrl": "https://graph.microsoft.com/v1.0/users",
        "tableName": "users",
        "enabled": true,
        "selectFields": [
          "id", "userPrincipalName", "displayName", "mail", 
          "jobTitle", "department", "accountEnabled"
        ],
        "filter": "accountEnabled eq true"
      },
      {
        "name": "security_groups",
        "endpointUrl": "https://graph.microsoft.com/v1.0/groups",
        "tableName": "groups",
        "enabled": true,
        "selectFields": [
          "id", "displayName", "description", "securityEnabled"
        ],
        "filter": "securityEnabled eq true"
      }
    ]
  }
}
```

This configuration will sync devices, active users, and security groups to separate tables in your database.
