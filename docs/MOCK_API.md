# Mock Graph API for Testing

This document describes the comprehensive mock Microsoft Graph API implementation for testing and development.

## Overview

The mock API provides a complete simulation of Microsoft Graph API responses, allowing for:
- **Offline development** without Azure connectivity
- **Controlled testing** with predictable data sets
- **Failure simulation** for resilience testing
- **Performance testing** with configurable delays

## Features

### 📊 **Realistic Data Generation**
- **Diverse device types**: Windows, macOS, Android, iOS devices
- **Authentic metadata**: Serial numbers, IMEI, compliance states
- **Dynamic updates**: Devices change over time like real environments
- **Pagination support**: Handles `$skip` and `$top` parameters

### 🎭 **Failure Simulation**
- **Rate limiting**: Simulate 429 responses with configurable probability
- **Authentication failures**: Test 401 scenarios
- **Network errors**: Simulate connection timeouts and failures
- **Response delays**: Configurable latency simulation

### 🔧 **Flexible Configuration**
- **Device count**: Generate any number of test devices
- **Failure rates**: Control probability of different error types
- **Response timing**: Simulate real-world API latency
- **Update frequency**: Control how often device data changes

## Configuration

Enable mock API in your `config.json`:

```json
{
  "mockGraphApi": {
    "enabled": true,
    "deviceCount": 100,
    "simulateRateLimits": false,
    "rateLimitProbability": 0.1,
    "simulateAuthFailures": false,
    "authFailureProbability": 0.05,
    "simulateNetworkErrors": false,
    "networkErrorProbability": 0.02,
    "responseDelayMs": [100, 500],
    "deviceUpdateFrequency": 0.1
  }
}
```

### Configuration Options

| Setting | Description | Default | Range |
|---------|-------------|---------|-------|
| `enabled` | Enable mock API mode | false | true/false |
| `deviceCount` | Number of devices to generate | 100 | 1-10000 |
| `simulateRateLimits` | Enable rate limit simulation | false | true/false |
| `rateLimitProbability` | Chance of rate limit response | 0.1 | 0.0-1.0 |
| `simulateAuthFailures` | Enable auth failure simulation | false | true/false |
| `authFailureProbability` | Chance of auth failure | 0.05 | 0.0-1.0 |
| `simulateNetworkErrors` | Enable network error simulation | false | true/false |
| `networkErrorProbability` | Chance of network error | 0.02 | 0.0-1.0 |
| `responseDelayMs` | Response delay range [min, max] | [100, 500] | [0, 10000] |
| `deviceUpdateFrequency` | How often devices change | 0.1 | 0.0-1.0 |

## Generated Device Data

### Device Types and Distribution

The mock API generates realistic device distributions:

```
Windows Devices (40%):
├── Manufacturers: Microsoft, Dell, HP, Lenovo
├── Types: Desktop, Laptop
└── OS Versions: Windows 10/11 builds

macOS Devices (25%):
├── Models: MacBook Pro, MacBook Air, iMac
├── OS Versions: macOS 12.x, 13.x, 14.x
└── Supervised status varies

Android Devices (20%):
├── Manufacturers: Samsung, Google, OnePlus
├── Models: Galaxy series, Pixel series
└── IMEI numbers included

iOS Devices (15%):
├── Models: iPhone 12, 13, 14, 15
├── OS Versions: iOS 15.x, 16.x, 17.x
└── IMEI numbers included
```

### Sample Device Object

```json
{
  "id": "12345678-1234-1234-1234-123456789012",
  "deviceName": "Microsoft-laptop-0001",
  "operatingSystem": "Windows",
  "osVersion": "10.0.19041.1000",
  "serialNumber": "SN00000001",
  "imei": null,
  "model": "Microsoft laptop",
  "manufacturer": "Microsoft",
  "enrolledDateTime": "2024-01-15T10:30:00.000Z",
  "lastSyncDateTime": "2024-06-02T14:25:00.000Z",
  "complianceState": "compliant",
  "azureADDeviceId": "87654321-4321-4321-4321-210987654321",
  "managedDeviceOwnerType": "company",
  "deviceType": "laptop",
  "deviceRegistrationState": "registered",
  "isEncrypted": true,
  "isSupervised": false,
  "emailAddress": "user1@company.com",
  "userDisplayName": "User 1",
  "userPrincipalName": "user1@company.com"
}
```

## API Endpoints

### Supported Endpoints

The mock API implements these Microsoft Graph endpoints:

#### **Get Managed Devices**
```
GET /deviceManagement/managedDevices
```

**Query Parameters:**
- `$skip`: Number of devices to skip (pagination)
- `$top`: Number of devices to return (max 1000)

**Response:**
```json
{
  "@odata.context": "https://graph.microsoft.com/v1.0/$metadata#deviceManagement/managedDevices",
  "@odata.count": 100,
  "value": [
    { /* device objects */ }
  ],
  "@odata.nextLink": "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices?$skip=50&$top=50"
}
```

#### **Get Device by ID**
```
GET /deviceManagement/managedDevices/{device-id}
```

**Response:**
```json
{
  "id": "device-id",
  "deviceName": "Device Name",
  /* ... other device properties ... */
}
```

## Testing Scenarios

### 1. **Basic Functionality Testing**

Test normal operation with mock data:
```json
{
  "mockGraphApi": {
    "enabled": true,
    "deviceCount": 50,
    "simulateRateLimits": false,
    "simulateAuthFailures": false,
    "simulateNetworkErrors": false,
    "responseDelayMs": [50, 100]
  }
}
```

### 2. **Rate Limiting Testing**

Test rate limit handling:
```json
{
  "mockGraphApi": {
    "enabled": true,
    "deviceCount": 100,
    "simulateRateLimits": true,
    "rateLimitProbability": 0.3,
    "responseDelayMs": [100, 300]
  }
}
```

### 3. **Resilience Testing**

Test error handling and recovery:
```json
{
  "mockGraphApi": {
    "enabled": true,
    "deviceCount": 200,
    "simulateRateLimits": true,
    "rateLimitProbability": 0.2,
    "simulateAuthFailures": true,
    "authFailureProbability": 0.1,
    "simulateNetworkErrors": true,
    "networkErrorProbability": 0.05
  }
}
```

### 4. **Performance Testing**

Test with large datasets and delays:
```json
{
  "mockGraphApi": {
    "enabled": true,
    "deviceCount": 5000,
    "simulateRateLimits": false,
    "responseDelayMs": [200, 800],
    "deviceUpdateFrequency": 0.2
  }
}
```

## Dynamic Device Updates

The mock API simulates real-world device changes:

### Update Types
- **Last sync time**: Updated on every request
- **Compliance state**: Changes randomly (10% chance)
- **OS version**: Occasional updates
- **Encryption status**: Rare changes

### Update Frequency
Controlled by `deviceUpdateFrequency`:
- `0.0`: No updates (static data)
- `0.1`: 10% chance of updates per request
- `0.5`: 50% chance of updates per request
- `1.0`: Updates on every request

## Error Simulation

### Rate Limit Responses (429)
```json
{
  "error": {
    "code": "TooManyRequests",
    "message": "Rate limit exceeded",
    "details": [
      {
        "code": "429",
        "message": "Too many requests"
      }
    ]
  }
}
```

### Authentication Failures (401)
```json
{
  "error": {
    "code": "Unauthorized",
    "message": "Authentication failed",
    "details": [
      {
        "code": "401",
        "message": "Invalid or expired token"
      }
    ]
  }
}
```

### Network Errors
- Connection timeouts
- DNS resolution failures
- Socket errors

## Monitoring Mock API

### Request Statistics

The mock API tracks:
- Total requests made
- Error responses generated
- Device updates performed
- Response times

### Log Messages

Mock API operations are logged:
```
INFO  Mock API: Generating 100 mock devices
DEBUG Mock API: Returning 50 devices (skip: 0, top: 50)
WARN  Mock API: Simulating rate limit response
ERROR Mock API: Simulating authentication failure
```

## Development Workflow

### 1. **Initial Development**
```bash
# Start with mock API for basic functionality
./IntuneDeviceDatabaseSynchronization run --config mock-config.json
```

### 2. **Integration Testing**
```bash
# Test with failure simulation
./IntuneDeviceDatabaseSynchronization run --config resilience-test-config.json
```

### 3. **Performance Testing**
```bash
# Test with large datasets
./IntuneDeviceDatabaseSynchronization run --config performance-test-config.json
```

### 4. **Production Validation**
```bash
# Final test with real API
./IntuneDeviceDatabaseSynchronization run --config production-config.json
```

## Best Practices

### 1. **Test Coverage**
- Test all failure scenarios before production
- Validate pagination with different device counts
- Test rate limiting behavior thoroughly

### 2. **Realistic Testing**
- Use device counts similar to production
- Set realistic response delays
- Enable appropriate failure rates

### 3. **Gradual Complexity**
- Start with simple scenarios
- Add failure simulation incrementally
- Test edge cases (empty responses, large datasets)

### 4. **Documentation**
- Document test scenarios and expected outcomes
- Keep test configurations in version control
- Share test results with team

## Switching Between Mock and Real API

### Configuration Toggle
Simply change the `enabled` flag:
```json
{
  "mockGraphApi": {
    "enabled": false  // Switch to real API
  }
}
```

### Environment-Based Configuration
Use different config files:
```bash
# Development with mock API
./app run --config config-dev.json

# Production with real API  
./app run --config config-prod.json
```

### Validation
Always validate configuration before switching:
```bash
./IntuneDeviceDatabaseSynchronization validate --config your-config.json
```
