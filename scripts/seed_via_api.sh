#!/usr/bin/env bash
set -euo pipefail

# seed_via_api.sh - Seed database via REST API
#
# Usage:
#   ./scripts/seed_via_api.sh                        # Use defaults
#   ./scripts/seed_via_api.sh API_URL=http://localhost:8080  # Custom API URL
#
# Environment variables:
#   API_URL     Base URL of backend-service API (default: http://localhost:8080)
#   SCHEMA_FILE Path to schema SQL file (default: db/schema.sql)
#   SEED_FILE   Path to seed SQL file (default: db/seed.sql)

# Configuration
API_URL="${API_URL:-http://localhost:8080}"
SCHEMA_FILE="${SCHEMA_FILE:-../db/schema.sql}"
SEED_FILE="${SEED_FILE:-../db/seed.sql}"

# Resolve paths relative to script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCHEMA_FILE="${SCRIPT_DIR}/${SCHEMA_FILE}"
SEED_FILE="${SCRIPT_DIR}/${SEED_FILE}"

# SQL execution endpoint
SQL_ENDPOINT="${API_URL}/api/sql"

# Check prerequisites
check_prerequisites() {
    if [[ ! -f "$SCHEMA_FILE" ]]; then
        echo "Error: Schema file not found at $SCHEMA_FILE"
        exit 1
    fi

    if [[ ! -f "$SEED_FILE" ]]; then
        echo "Error: Seed file not found at $SEED_FILE"
        exit 1
    fi
}

# Check if API is available
check_api() {
    echo "[seed_via_api] Checking API availability at $API_URL..."

    local health_response
    if health_response=$(curl -s "${API_URL}/api/health" 2>/dev/null); then
        if echo "$health_response" | grep -q "healthy"; then
            echo "[seed_via_api] API is healthy"
            return 0
        fi
    fi

    echo "Warning: API health check failed or returned unexpected response"
    echo "         Trying to proceed anyway..."
    echo ""
}

# Execute SQL file via API
execute_sql_file_via_api() {
    local file="$1"
    local description="$2"

    echo "[seed_via_api] $description: $file"

    # Read the SQL file and send it to the API endpoint
    local sql_content
    sql_content=$(cat "$file")

    # Split by semicolons and execute each statement
    # This handles multiple statements in one file
    local statements
    statements=$(echo "$sql_content" | sed 's/;[[:space:]]*/;\n/g' | grep -v '^[[:space:]]*$')

    local count=0
    local errors=0

    while IFS= read -r stmt; do
        # Skip empty statements and comments
        [[ -z "$stmt" ]] && continue
        [[ "$stmt" =~ ^[[:space:]]*-- ]] && continue

        ((count++))

        local response
        response=$(curl -s -X POST "$SQL_ENDPOINT" \
            -H "Content-Type: application/json" \
            -d "{\"sql\": \"$stmt\"}" 2>&1)

        # Check for errors (simple check - look for error_code or "Error" in response)
        if echo "$response" | grep -q '"error_code"'; then
            ((errors++))
            echo "  [WARN] Statement $count failed: ${response:0:100}..."
        fi
    done <<< "$statements"

    echo "[seed_via_api] $description complete ($count statements, $errors errors)"
}

# Alternative: execute file as single request (for engines that support multiple statements)
execute_sql_file_single() {
    local file="$1"
    local description="$2"

    echo "[seed_via_api] $description (single request): $file"

    local sql_content
    sql_content=$(cat "$file" | tr '\n' ' ' | sed 's/[[:space:]]+/ /g')

    local response
    response=$(curl -s -X POST "$SQL_ENDPOINT" \
        -H "Content-Type: application/json" \
        -d "{\"sql\": \"$sql_content\"}")

    if echo "$response" | grep -q '"error_code"'; then
        echo "[seed_via_api] Warning: Some statements may have failed"
        echo "Response: $response"
    else
        echo "[seed_via_api] $description complete"
    fi
}

# Main execution
main() {
    echo "[seed_via_api] Starting API-driven database seeding"
    echo "[seed_via_api] API URL: $API_URL"
    echo ""

    check_prerequisites
    check_api

    # Apply schema
    execute_sql_file_single "$SCHEMA_FILE" "Applying schema"

    # Apply seed data
    echo ""
    execute_sql_file_single "$SEED_FILE" "Applying seed data"

    echo ""
    echo "[seed_via_api] Database seeding complete!"
    echo ""
    echo "You can verify the data with:"
    echo "  curl -s $SQL_ENDPOINT -H 'Content-Type: application/json' -d '{\"sql\":\"SELECT COUNT(*) FROM users\"}'"
}

main "$@"
