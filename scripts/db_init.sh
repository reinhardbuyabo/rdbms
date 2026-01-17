#!/usr/bin/env bash
set -euo pipefail

# db_init.sh - Initialize database with schema and seed data
#
# Usage:
#   ./scripts/db_init.sh                     # Use defaults
#   ./scripts/db_init.sh DB_PATH=./data.db  # Custom database path
#   ./scripts/db_init.sh REPL=./target/release/rdbms  # Custom REPL binary
#
# Environment variables:
#   DB_PATH      Path to database file (default: ./data.db)
#   REPL         Path to RDBMS REPL binary (default: ./target/debug/rdbms)
#   SCHEMA_FILE  Path to schema SQL file (default: db/schema.sql)
#   SEED_FILE    Path to seed SQL file (default: db/seed.sql)

# Configuration
DB_PATH="${DB_PATH:-./data.db}"
REPL="${REPL:-./target/debug/rdbms}"
SCHEMA_FILE="${SCHEMA_FILE:-../db/schema.sql}"
SEED_FILE="${SEED_FILE:-../db/seed.sql}"

# Resolve paths relative to script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCHEMA_FILE="${SCRIPT_DIR}/${SCHEMA_FILE}"
SEED_FILE="${SCRIPT_DIR}/${SEED_FILE}"

# Check prerequisites
check_prerequisites() {
    if [[ ! -f "$REPL" ]]; then
        echo "Error: REPL binary not found at $REPL"
        echo "Build it first with: make build"
        exit 1
    fi

    if [[ ! -f "$SCHEMA_FILE" ]]; then
        echo "Error: Schema file not found at $SCHEMA_FILE"
        exit 1
    fi

    if [[ ! -f "$SEED_FILE" ]]; then
        echo "Error: Seed file not found at $SEED_FILE"
        exit 1
    fi
}

# Execute SQL file using the REPL
execute_sql_file() {
    local file="$1"
    local description="$2"

    echo "[db_init] $description: $file"

    # The REPL reads from stdin, so we pipe the SQL file to it
    # Suppress REPL startup messages and just execute
    cat "$file" | "$REPL" --db "$DB_PATH" 2>/dev/null || {
        echo "Warning: Some statements may have failed or produced output"
    }

    echo "[db_init] $description complete"
}

# Main execution
main() {
    echo "[db_init] Starting database initialization"
    echo "[db_init] Database: $DB_PATH"
    echo "[db_init] REPL: $REPL"
    echo ""

    check_prerequisites

    # Ensure parent directory exists
    local db_dir
    db_dir=$(dirname "$DB_PATH")
    if [[ "$db_dir" != "." ]] && [[ ! -d "$db_dir" ]]; then
        echo "[db_init] Creating database directory: $db_dir"
        mkdir -p "$db_dir"
    fi

    # Apply schema
    echo ""
    execute_sql_file "$SCHEMA_FILE" "Applying schema"

    # Apply seed data
    echo ""
    execute_sql_file "$SEED_FILE" "Applying seed data"

    echo ""
    echo "[db_init] Database initialization complete!"
    echo ""
    echo "You can verify the data with:"
    echo "  $REPL --db $DB_PATH -c 'SELECT COUNT(*) FROM users'"
    echo "  $REPL --db $DB_PATH -c 'SELECT COUNT(*) FROM events'"
}

main "$@"
