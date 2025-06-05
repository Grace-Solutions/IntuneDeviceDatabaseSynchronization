#!/bin/bash
set -e

# Create data directory if it doesn't exist
mkdir -p /app/data/logs /app/data/backups

# Copy config template if config doesn't exist
if [ ! -f /app/data/config.json ]; then
    echo "Config file not found, copying template..."
    cp /app/config.json.template /app/data/config.json
    echo "Please edit /app/data/config.json with your Azure credentials and settings"
fi

# Run the application with config from data directory
exec intune-device-sync --config /app/data/config.json "$@"
