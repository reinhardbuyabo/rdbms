#!/bin/bash
set -e

API_URL="${API_URL:-http://localhost:8080}"
DB_DIR="${DB_DIR:-/db}"
MAX_RETRIES=30
RETRY_INTERVAL=2

wait_for_api() {
    echo "Waiting for backend-service at $API_URL..."
    retries=0
    while [ $retries -lt $MAX_RETRIES ]; do
        if curl -s "$API_URL/api/health" > /dev/null 2>&1; then
            echo "Backend-service is ready!"
            return 0
        fi
        retries=$((retries + 1))
        echo "Attempt $retries/$MAX_RETRIES - waiting ${RETRY_INTERVAL}s..."
        sleep $RETRY_INTERVAL
    done
    echo "Error: Backend-service did not become ready in time"
    exit 1
}

wait_for_api

python3 /usr/local/bin/docker-init.py
