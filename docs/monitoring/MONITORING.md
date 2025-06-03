# Monitoring Guide

## Overview

IntuneDeviceDatabaseSynchronization provides comprehensive monitoring capabilities through Prometheus metrics and optional webhook notifications. This guide covers setting up monitoring dashboards and alerting.

## Prometheus Metrics

### Enabling Metrics

Configure metrics in `config.json`:

```json
{
  "enablePrometheus": true,
  "prometheusPort": 9898
}
```

Metrics are available at: `http://localhost:9898/metrics`

### Available Metrics

#### Sync Operations
- `sync_success_total` - Total successful sync operations
- `sync_failure_total` - Total failed sync operations  
- `sync_duration_seconds` - Duration of sync operations

#### Device Processing
- `devices_fetched_total` - Total devices fetched from Intune
- `devices_processed_total` - Total devices processed
- `devices_current_count` - Current number of devices in database
- `device_filter_matched_total` - Devices allowed by OS filter
- `device_filter_skipped_total` - Devices skipped by OS filter

#### Database Operations
- `db_insert_total` - Database insert operations
- `db_update_total` - Database update operations
- `db_skip_total` - Database operations skipped (no changes)
- `db_error_total` - Database errors

#### Authentication & HTTP
- `token_refresh_total` - OAuth token refresh operations
- `auth_failure_total` - Authentication failures
- `http_requests_total` - HTTP requests made
- `http_errors_total` - HTTP errors

#### System Metrics
- `process_start_time_seconds` - Service start time
- `process_cpu_seconds_total` - CPU usage
- `process_memory_bytes` - Memory usage

## Grafana Dashboard

### Installation

1. **Import Dashboard**:
   - Copy `docs/monitoring/grafana-dashboard.json`
   - In Grafana: Dashboards → Import → Paste JSON

2. **Configure Data Source**:
   - Add Prometheus data source pointing to your Prometheus server
   - URL: `http://prometheus-server:9090`

### Dashboard Features

The included dashboard provides:

#### Overview Panels
- **Sync Operations Overview** - Success/failure counts
- **Current Device Count** - Total devices with thresholds
- **Service Uptime** - How long the service has been running

#### Performance Monitoring
- **Sync Duration** - Time taken for sync operations
- **Device Processing Rate** - Devices processed per second
- **Database Operations** - Insert/update/skip rates

#### Error Tracking
- **Error Rate** - Percentage of failed operations
- **Database Errors** - Database operation failures
- **HTTP Errors** - API communication issues

#### Filtering Statistics
- **Device OS Filter** - Pie chart of matched vs filtered devices
- **Authentication Metrics** - Token refresh and auth failure rates

### Customization

#### Adding Custom Panels

1. **Device Count by OS**:
   ```promql
   sum by (os) (devices_current_count)
   ```

2. **Average Sync Duration**:
   ```promql
   avg_over_time(sync_duration_seconds[1h])
   ```

3. **Database Connection Pool**:
   ```promql
   db_connections_active
   db_connections_idle
   ```

#### Setting Up Alerts

Create alerts for critical metrics:

1. **High Error Rate**:
   ```promql
   rate(sync_failure_total[5m]) / rate(sync_success_total[5m] + sync_failure_total[5m]) > 0.1
   ```

2. **Sync Duration Too Long**:
   ```promql
   sync_duration_seconds > 300
   ```

3. **No Recent Syncs**:
   ```promql
   time() - sync_success_total > 7200
   ```

## Webhook Notifications

### Configuration

Add webhook configuration to `config.json`:

```json
{
  "webhook": {
    "enabled": true,
    "url": "https://your-webhook-endpoint.com/intune-sync",
    "timeout_seconds": 30,
    "retry_attempts": 3,
    "retry_delay_seconds": 5,
    "events": [
      "sync_started",
      "sync_completed", 
      "sync_failed",
      "devices_updated",
      "database_error",
      "authentication_failed"
    ],
    "headers": {
      "Authorization": "Bearer your-token",
      "X-Service": "IntuneDeviceSync"
    },
    "secret": "your-webhook-secret"
  }
}
```

### Webhook Events

#### Sync Events
- **sync_started** - Sync operation begins
- **sync_completed** - Sync operation completes successfully
- **sync_failed** - Sync operation fails

#### Data Events  
- **devices_updated** - Device data changes detected
- **database_error** - Database operation fails
- **authentication_failed** - OAuth authentication fails

### Webhook Payload Format

```json
{
  "event": "sync_completed",
  "timestamp": "2025-06-02T22:30:00Z",
  "service": "IntuneDeviceDatabaseSynchronization",
  "version": "2025.06.02.2230",
  "data": {
    "sync_id": "sync-12345",
    "duration_seconds": 45.2,
    "devices_fetched": 1250,
    "devices_updated": 15,
    "devices_inserted": 3,
    "devices_skipped": 1232
  }
}
```

### Security

Webhooks include HMAC-SHA256 signatures when a secret is configured:

```
X-Webhook-Signature: sha256=abc123...
```

Verify signatures in your webhook handler:

```python
import hmac
import hashlib

def verify_signature(payload, signature, secret):
    expected = hmac.new(
        secret.encode(),
        payload.encode(),
        hashlib.sha256
    ).hexdigest()
    return hmac.compare_digest(f"sha256={expected}", signature)
```

## Alerting Setup

### Prometheus Alertmanager

Create alerting rules in `prometheus-rules.yml`:

```yaml
groups:
- name: intune-sync-alerts
  rules:
  - alert: IntuneSyncFailed
    expr: increase(sync_failure_total[5m]) > 0
    for: 0m
    labels:
      severity: critical
    annotations:
      summary: "Intune sync operation failed"
      description: "Sync failure detected in the last 5 minutes"

  - alert: IntuneSyncHighDuration
    expr: sync_duration_seconds > 300
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "Intune sync taking too long"
      description: "Sync duration is {{ $value }} seconds"

  - alert: IntuneServiceDown
    expr: up{job="intune-device-sync"} == 0
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "Intune sync service is down"
      description: "Service has been down for more than 1 minute"

  - alert: IntuneHighErrorRate
    expr: rate(sync_failure_total[10m]) / rate(sync_success_total[10m] + sync_failure_total[10m]) > 0.1
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High error rate in Intune sync"
      description: "Error rate is {{ $value | humanizePercentage }}"
```

### Notification Channels

Configure Alertmanager to send notifications:

```yaml
route:
  group_by: ['alertname']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 1h
  receiver: 'web.hook'

receivers:
- name: 'web.hook'
  webhook_configs:
  - url: 'https://your-notification-endpoint.com/alerts'
    send_resolved: true

- name: 'slack'
  slack_configs:
  - api_url: 'https://hooks.slack.com/services/...'
    channel: '#monitoring'
    title: 'Intune Sync Alert'
    text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'
```

## Log Monitoring

### Structured Logging

The service outputs structured logs that can be parsed by log aggregation systems:

```json
{
  "timestamp": "2025-06-02T22:30:00Z",
  "level": "INFO",
  "component": "Sync",
  "message": "Sync operation completed",
  "sync_id": "sync-12345",
  "duration_ms": 45200,
  "devices_processed": 1250
}
```

### ELK Stack Integration

Configure Filebeat to ship logs to Elasticsearch:

```yaml
filebeat.inputs:
- type: log
  enabled: true
  paths:
    - /opt/intune-device-sync/logs/*.log
  json.keys_under_root: true
  json.add_error_key: true

output.elasticsearch:
  hosts: ["elasticsearch:9200"]
  index: "intune-device-sync-%{+yyyy.MM.dd}"
```

### Log-based Alerts

Create alerts based on log patterns:

```json
{
  "query": {
    "bool": {
      "must": [
        {"match": {"level": "ERROR"}},
        {"range": {"@timestamp": {"gte": "now-5m"}}}
      ]
    }
  }
}
```

## Performance Monitoring

### Key Performance Indicators

Monitor these KPIs for optimal performance:

1. **Sync Frequency** - Operations per hour
2. **Sync Duration** - Average and 95th percentile
3. **Error Rate** - Percentage of failed operations
4. **Device Processing Rate** - Devices per second
5. **Database Performance** - Query duration and connection pool usage

### Capacity Planning

Use metrics to plan for growth:

- **Device Growth Rate** - Track device count increases
- **Resource Usage** - Monitor CPU and memory trends
- **Database Size** - Track database growth over time
- **Network Usage** - Monitor API call frequency and data transfer

### Optimization Recommendations

Based on metrics, consider:

1. **Increase Sync Interval** if processing is slow
2. **Add Database Indexes** if query performance degrades
3. **Scale Horizontally** if single instance reaches limits
4. **Optimize Filters** to reduce unnecessary processing
