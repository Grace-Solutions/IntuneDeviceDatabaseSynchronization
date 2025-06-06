# MSGraphDBSynchronizer v2025.06.06.0204 - Dynamic Multi-Endpoint Support

## 🆕 New Features

### **Dynamic Endpoint Support**
- **🎯 Configurable Object Counts**: Individual `mockObjectCount` settings for each endpoint
- **📊 Per-Endpoint Data**: Realistic mock data generation tailored to each endpoint type
- **🔧 Automatic Schema Evolution**: Dynamic table creation and column mapping based on endpoint data
- **📱 Serial Number Device Names**: Real-world device naming using manufacturer-specific serial numbers
- **⏰ Timestamp Versioning**: Proper yyyy.MM.dd.HHmm format for build tracking

### **Enhanced Mock Data Quality**
- **👥 Users**: Realistic names, departments, job titles, and email addresses
- **🏢 Groups**: Security, Distribution, Microsoft 365, and Dynamic groups with proper settings
- **💻 Devices**: Enterprise-grade device data with proper serial number formats
- **🔗 Relationships**: Proper data relationships between endpoints

## ✅ Core Features

- **🔄 Microsoft Graph Integration**: Sync any Graph API endpoint with OAuth2 authentication
- **🌐 Multi-Endpoint Support**: Sync devices, users, groups, compliance policies, and custom endpoints
- **🎛️ Advanced OS Filtering**: Wildcard support with case-insensitive substring matching
- **💾 Multi-Database Support**: SQLite, PostgreSQL, and MSSQL backends with automatic schema creation
- **📊 Prometheus Metrics**: Comprehensive monitoring and observability
- **🖥️ Cross-Platform**: Native binaries for Windows, Linux, and macOS
- **🛠️ Service Management**: Windows service, systemd, and launchd support
- **⚙️ Flexible Configuration**: JSON config with environment variable overrides
- **🚦 Rate Limiting**: Intelligent API rate limiting with exponential backoff retry logic
- **🧪 Mock API**: Complete Graph API simulation for testing and development

## 📦 Installation

1. **Download** the release package for your platform
2. **Extract** the ZIP file
3. **Edit** `config.json` with your Azure app credentials
4. **Run**: `MSGraphDBSynchronizer.exe run`

## 🚀 Quick Start

```json
{
  "clientId": "your-azure-client-id",
  "clientSecret": "your-azure-client-secret", 
  "tenantId": "your-azure-tenant-id",
  "endpoints": {
    "endpoints": [
      {
        "name": "Devices",
        "enabled": true,
        "mockObjectCount": 30000
      },
      {
        "name": "Users", 
        "enabled": true,
        "mockObjectCount": 5000
      },
      {
        "name": "Groups",
        "enabled": true,
        "mockObjectCount": 1000
      }
    ]
  },
  "mockGraphApi": {
    "enabled": true
  }
}
```

## 📊 Performance

- **32,000+ objects** processed efficiently across multiple endpoints
- **Enterprise-scale** mock data generation
- **Real-time sync** with rate limiting and retry logic
- **Automatic schema** creation and evolution

## 🔧 Service Management

```bash
# Install Windows service
MSGraphDBSynchronizer.exe install

# Start service
MSGraphDBSynchronizer.exe start

# Check status
MSGraphDBSynchronizer.exe status
```

## 📈 Monitoring

Access Prometheus metrics at: `http://localhost:9898/metrics`

## 📚 Documentation

- **Repository**: https://github.com/Grace-Solutions/MSGraphDBSynchronizer
- **License**: GPL-3.0
- **Support**: GitHub Issues

---

**Full Changelog**: https://github.com/Grace-Solutions/MSGraphDBSynchronizer/compare/v2025.06.02.2230...v2025.06.06.0204
