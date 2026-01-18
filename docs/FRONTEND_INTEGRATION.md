# Frontend Integration Guide

This guide provides comprehensive information for frontend developers integrating with the RDBMS backend service.

## Overview

The RDBMS backend service provides a REST API for frontend application integration. It supports SQL execution, transactions, and authentication via Google OAuth.

## API Endpoints

### Health Check

```bash
GET /api/health

# Response
{"status":"healthy","version":"0.2.0"}
```

### Execute SQL

```bash
POST /api/sql
Content-Type: application/json

{
  "sql": "SELECT * FROM users",
  "tx_id": "optional-transaction-id"
}

# Response (success)
{
  "columns": ["id", "name", "email"],
  "rows": [
    [{"type":"int","value":1}, {"type":"text","value":"Alice"}, {"type":"text","value":"alice@example.com"}],
    [{"type":"int","value":2}, {"type":"text","value":"Bob"}, {"type":"text","value":"bob@example.com"}]
  ],
  "rows_affected": 2,
  "message": null
}

# Response (error)
{
  "error_code": "SQL_PARSE_ERROR",
  "message": "syntax error at position 15"
}
```

### Transaction Management

```bash
# Begin transaction
POST /api/tx/begin
Content-Type: application/json

# Response
{
  "tx_id": "550e8400-e29b-41d4-a716-446655440000",
  "message": "Transaction started"
}

# Commit transaction
POST /api/tx/{tx_id}/commit

# Abort transaction (rollback)
POST /api/tx/{tx_id}/abort
```

## Data Types

The API returns typed values in a consistent format:

| Type | Format | Example |
|------|--------|---------|
| INT | `{"type":"int","value":42}` | Integer values |
| TEXT | `{"type":"text","value":"hello"}` | String values |
| NULL | `{"type":"Null"}` | NULL values |

### Handling Responses

**JavaScript/TypeScript:**
```javascript
function parseRow(row) {
  return row.map(cell => {
    switch (cell.type) {
      case 'int': return Number(cell.value);
      case 'text': return cell.value;
      case 'Null': return null;
      default: return cell.value;
    }
  });
}

async function query(sql) {
  const response = await fetch('/api/sql', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ sql })
  });
  const result = await response.json();
  
  if (result.error_code) {
    throw new Error(result.message);
  }
  
  return result.rows.map(parseRow);
}
```

**Python:**
```python
import json
import requests

def query(sql):
    response = requests.post(
        'http://localhost:8080/api/sql',
        json={'sql': sql}
    )
    result = response.json()
    
    if 'error_code' in result:
        raise Exception(result['message'])
    
    def parse_cell(cell):
        if cell['type'] == 'int':
            return int(cell['value'])
        elif cell['type'] == 'text':
            return cell['value']
        elif cell['type'] == 'Null':
            return None
        return cell['value']
    
    return [list(map(parse_cell, row)) for row in result['rows']]
```

## Authentication

### Google OAuth 2.0 Flow

```javascript
// Step 1: Redirect user to Google
window.location.href = 'http://localhost:8080/auth/google/start';

// Step 2: After Google redirects back with code
// The server handles the callback and sets a JWT cookie

// Step 3: Access protected endpoints
const response = await fetch('/v1/users/me', {
  headers: {
    'Authorization': 'Bearer ' + jwtToken
  }
});
```

### Protected Endpoints

The following endpoints require Bearer token authentication:

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/v1/users/me` | Get current user profile |
| `PATCH` | `/v1/users/me` | Update current user profile |
| `POST` | `/v1/users/me/role` | Update user role |

**Public OAuth endpoints:**
- `GET /auth/google/start` - Start OAuth flow (public, redirects to Google)
- `GET /auth/google/callback` - OAuth callback (public, handles Google redirect)

## Transaction Workflow

For operations requiring atomicity (e.g., payments, bookings):

```javascript
async function transferFunds(fromId, toId, amount) {
  // Begin transaction
  const beginRes = await fetch('/api/tx/begin', { method: 'POST' });
  const { tx_id } = await beginRes.json();
  
  try {
    // Execute operations within transaction
    await query(`UPDATE accounts SET balance = balance - ${amount} WHERE id = ${fromId}`, tx_id);
    await query(`UPDATE accounts SET balance = balance + ${amount} WHERE id = ${toId}`, tx_id);
    
    // Verify balances within transaction
    const balances = await query('SELECT id, balance FROM accounts', tx_id);
    
    // Commit if everything looks good
    await fetch(`/api/tx/${tx_id}/commit`, { method: 'POST' });
    return true;
  } catch (error) {
    // Abort on any error
    await fetch(`/api/tx/${tx_id}/abort`, { method: 'POST' });
    throw error;
  }
}
```

## Error Handling

### Error Codes

| Code | Description |
|------|-------------|
| `SQL_PARSE_ERROR` | Invalid SQL syntax |
| `CATALOG_ERROR` | Table or column not found |
| `EXECUTION_ERROR` | Query execution failed |
| `TRANSACTION_ERROR` | Transaction conflict or timeout |
| `AUTH_REQUIRED` | Authentication required |
| `INVALID_TOKEN` | Invalid or expired JWT |

### Best Practices

1. **Always handle errors:**
```javascript
try {
  const result = await query(sql);
  displayResults(result);
} catch (error) {
  showError(error.message);
  if (error.message.includes('UNIQUE constraint')) {
    showToast('This record already exists');
  }
}
```

2. **Validate input before sending:**
```javascript
function validateUserInput(name, email) {
  if (!name || name.trim().length === 0) {
    throw new Error('Name is required');
  }
  if (!email.includes('@')) {
    throw new Error('Invalid email format');
  }
}
```

3. **Use parameterized queries (via SQL parser):**
```javascript
// Escape single quotes in values
function escapeSQL(value) {
  return value.replace(/'/g, "''");
}

const safeName = escapeSQL(userInputName);
await query(`INSERT INTO users (name) VALUES ('${safeName}')`);
```

## Performance Considerations

### Connection Management

- The REST API is stateless; no persistent connections needed
- Each request is independent
- For high-throughput scenarios, consider connection pooling on the client

### Query Optimization

1. **Use indexes:**
```sql
-- Create index for frequently queried columns
CREATE INDEX idx_users_email ON users(email);
```

2. **Limit result sets:**
```sql
-- Always use LIMIT for large tables
SELECT * FROM logs ORDER BY created_at DESC LIMIT 100;
```

3. **Avoid SELECT *** in production:
```sql
-- Instead of: SELECT * FROM users
-- Use: SELECT id, name, email FROM users
```

## Current Limitations

### Known Issues

**Integration Tests Failing:**
Some backend integration tests are currently failing due to test isolation issues. This does not affect the core functionality but requires manual attention when updating tests.

**IMPORTANT:** When modifying integration tests, ensure each test:
- Creates its own test database (use `tempfile`)
- Properly cleans up resources after completion
- Uses unique identifiers to avoid conflicts with other tests

See `services/backend-service/tests/integration_tests.rs` for the current test implementation.

### Unsupported Features

- JOINs with complex conditions
- Subqueries in WHERE clause
- Aggregates (COUNT, SUM, AVG, etc.)
- ORDER BY with multiple columns
- LIMIT in UPDATE/DELETE

### Workarounds

For features not yet implemented:

1. **Client-side aggregation:**
```javascript
// Instead of SELECT SUM(amount) FROM orders
const result = await query('SELECT amount FROM orders');
const sum = result.rows.reduce((acc, row) => acc + row[0].value, 0);
```

2. **Client-side filtering:**
```javascript
// Instead of complex WHERE clauses
const result = await query('SELECT * FROM products');
const filtered = result.rows.filter(row => 
  row.some(cell => cell.value >= 100 && cell.value <= 500)
);
```

## Development Setup

### Running Locally with Docker Compose

```bash
# Start services
docker compose up -d

# Check status
docker compose ps

# View logs
docker compose logs -f backend-service

# Run database initialization
docker compose run --rm db-init

# Stop services
docker compose down
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | 8080 | HTTP server port |
| `DB_PATH` | ./data.db | Database file path |
| `BIND` | 0.0.0.0 | Bind address |
| `JWT_SECRET` | - | JWT signing secret (required for auth) |
| `GOOGLE_CLIENT_ID` | - | Google OAuth client ID |
| `GOOGLE_CLIENT_SECRET` | - | Google OAuth client secret |

## Debugging Tips

### Enable Verbose Logging

```bash
# Run with RUST_LOG for debug output
RUST_LOG=debug ./target/release/backend-service --db ./mydb --port 8080
```

### Test Database Operations

```bash
# Using curl
curl -X POST http://localhost:8080/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"SELECT 1 as test"}'

# Using the REPL
./target/release/rdbms --db ./data.db
```

## Related Documentation

- [QUICKSTART.md](../QUICKSTART.md) - Getting started guide
- [postman-testing-guide.md](./postman-testing-guide.md) - API testing with Postman
- [TRANSACTION_INVESTIGATION.md](./TRANSACTION_INVESTIGATION.md) - Transaction debugging notes
