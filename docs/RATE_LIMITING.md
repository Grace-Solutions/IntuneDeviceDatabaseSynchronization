# Rate Limiting and Retry Logic

This document describes the comprehensive rate limiting and retry mechanisms implemented in the Intune Device Database Synchronization service.

## Overview

The service implements intelligent rate limiting to respect Microsoft Graph API limits and handle rate limit responses gracefully with exponential backoff retry logic.

## Features

### 🚦 **Proactive Rate Limiting**
- **Request tracking**: Monitors requests per minute window
- **Automatic throttling**: Prevents exceeding configured limits
- **Sliding window**: Uses 60-second rolling window for accurate tracking

### 🔄 **Intelligent Retry Logic**
- **Exponential backoff**: Progressively longer delays between retries
- **Jitter support**: Randomized delays to prevent thundering herd
- **Server-guided delays**: Respects `Retry-After` headers from API responses
- **Maximum retry limits**: Configurable maximum attempts

### 📊 **Rate Limit Monitoring**
- **Real-time statistics**: Current request count and remaining capacity
- **Consecutive failures**: Tracks rate limit occurrences
- **Performance metrics**: Integration with Prometheus monitoring

## Configuration

Add rate limiting configuration to your `config.json`:

```json
{
  "rateLimit": {
    "maxRequestsPerMinute": 60,
    "initialRetryDelaySeconds": 1,
    "maxRetryDelaySeconds": 300,
    "maxRetryAttempts": 5,
    "backoffMultiplier": 2.0,
    "enableJitter": true
  }
}
```

### Configuration Options

| Setting | Description | Default | Range |
|---------|-------------|---------|-------|
| `maxRequestsPerMinute` | Maximum API requests per minute | 60 | 1-1000 |
| `initialRetryDelaySeconds` | Initial delay before first retry | 1 | 1-60 |
| `maxRetryDelaySeconds` | Maximum delay between retries | 300 | 60-3600 |
| `maxRetryAttempts` | Maximum number of retry attempts | 5 | 1-10 |
| `backoffMultiplier` | Exponential backoff multiplier | 2.0 | 1.0-10.0 |
| `enableJitter` | Add randomization to delays | true | true/false |

## How It Works

### 1. **Request Tracking**
```
┌─────────────────────────────────────────────────────────┐
│ Sliding Window (60 seconds)                            │
├─────────────────────────────────────────────────────────┤
│ [Req1] [Req2] [Req3] ... [ReqN]                       │
│   ↓      ↓      ↓         ↓                           │
│ Track timestamps for rate limit calculation            │
└─────────────────────────────────────────────────────────┘
```

### 2. **Exponential Backoff**
```
Attempt 1: 1 second delay
Attempt 2: 2 seconds delay  (1 × 2.0)
Attempt 3: 4 seconds delay  (2 × 2.0)
Attempt 4: 8 seconds delay  (4 × 2.0)
Attempt 5: 16 seconds delay (8 × 2.0)
```

### 3. **Jitter Calculation**
```
Base Delay: 4 seconds
Jitter Range: ±20% (3.2 - 4.8 seconds)
Final Delay: Random value in range
```

## API Integration

### Automatic Rate Limiting

The service automatically applies rate limiting to all Microsoft Graph API calls:

```rust
// Example: Automatic rate limiting
let rate_limiter = RateLimiter::new(config.rate_limit);

// This call will wait if rate limit is reached
rate_limiter.acquire_permit().await?;

// Make API request
let response = client.get("https://graph.microsoft.com/v1.0/deviceManagement/managedDevices")
    .send()
    .await?;
```

### Handling Rate Limit Responses

When the API returns a 429 (Too Many Requests) response:

```rust
match response.status() {
    429 => {
        // Parse Retry-After header
        let retry_after = response.headers()
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs);

        // Handle rate limit with server guidance
        let delay = rate_limiter.handle_rate_limit_response(retry_after).await?;
        
        // Wait before retrying
        tokio::time::sleep(delay).await;
    }
    _ => { /* Handle other responses */ }
}
```

## Monitoring and Metrics

### Prometheus Metrics

The service exposes rate limiting metrics:

```
# Current requests in the sliding window
intune_sync_rate_limit_current_requests

# Requests remaining before hitting limit
intune_sync_rate_limit_requests_remaining

# Total rate limit events
intune_sync_rate_limit_events_total

# Rate limit retry attempts
intune_sync_rate_limit_retries_total
```

### Log Messages

Rate limiting events are logged with appropriate levels:

```
INFO  Rate limit reached, waiting 2.3s before next request
WARN  Rate limited by API (attempt 2), backing off for 4.1s
ERROR Maximum retry attempts exceeded for rate limiting
```

## Best Practices

### 1. **Conservative Limits**
- Start with lower `maxRequestsPerMinute` (30-60)
- Monitor actual API usage patterns
- Gradually increase if needed

### 2. **Appropriate Timeouts**
- Set `maxRetryDelaySeconds` based on sync frequency
- For hourly syncs: 300 seconds (5 minutes) max
- For daily syncs: 1800 seconds (30 minutes) max

### 3. **Jitter Configuration**
- Always enable jitter in production
- Helps prevent synchronized retry storms
- Reduces load on API endpoints

### 4. **Monitoring Setup**
- Set up alerts for high rate limit events
- Monitor retry attempt patterns
- Track API response times

## Troubleshooting

### Common Issues

#### **Frequent Rate Limiting**
```
Symptoms: Many "Rate limit reached" log messages
Cause: maxRequestsPerMinute too high
Solution: Reduce the limit or increase poll interval
```

#### **Long Sync Times**
```
Symptoms: Sync operations taking very long
Cause: Aggressive rate limiting or many retries
Solution: Optimize rate limit settings or check API health
```

#### **Retry Exhaustion**
```
Symptoms: "Maximum retry attempts exceeded" errors
Cause: Persistent API issues or too low retry limits
Solution: Increase maxRetryAttempts or check API status
```

### Diagnostic Commands

Check rate limit statistics:
```bash
# View current rate limit status in logs
tail -f logs/intune-sync.log | grep "rate.limit"

# Check Prometheus metrics
curl http://localhost:9898/metrics | grep rate_limit
```

## Microsoft Graph API Limits

### Standard Limits
- **Application requests**: 10,000 requests per 10 minutes
- **Per-app per-tenant**: 10,000 requests per 10 minutes
- **Device management**: Additional throttling may apply

### Rate Limit Headers
Microsoft Graph returns these headers:
- `Retry-After`: Seconds to wait before retrying
- `X-RateLimit-Limit`: Request limit for the time window
- `X-RateLimit-Remaining`: Remaining requests in window
- `X-RateLimit-Reset`: Time when the limit resets

## Advanced Configuration

### Environment Variables
Override configuration via environment variables:
```bash
export RATE_LIMIT_MAX_REQUESTS_PER_MINUTE=30
export RATE_LIMIT_MAX_RETRY_ATTEMPTS=3
export RATE_LIMIT_ENABLE_JITTER=true
```

### Dynamic Adjustment
The service can dynamically adjust rate limits based on API responses:
```json
{
  "rateLimit": {
    "adaptiveMode": true,
    "minRequestsPerMinute": 10,
    "maxRequestsPerMinute": 100,
    "adjustmentFactor": 0.8
  }
}
```

## Testing Rate Limiting

Use the mock API to test rate limiting behavior:
```json
{
  "mockGraphApi": {
    "enabled": true,
    "simulateRateLimits": true,
    "rateLimitProbability": 0.3
  }
}
```

This will simulate rate limit responses 30% of the time for testing.
