# Postman Testing Guide for RDBMS REST API

This document provides complete instructions for testing the RDBMS REST API service using Postman. All tests are organized by functionality and include both individual requests and full test collections.

## 📋 Table of Contents

1. [Setup](#setup)
2. [Basic Functionality Tests](#basic-functionality-tests)
3. [Transaction Management Tests](#transaction-management-tests)
4. [Error Handling Tests](#error-handling-tests)
5. [Concurrency Tests](#concurrency-tests)
6. [Complete Test Collection](#complete-test-collection)
7. [Expected Results](#expected-results)
8. [Troubleshooting](#troubleshooting)

---

## 🔧 Setup

### 1. Import Environment Variables

In Postman, create the following environment variables:

| Variable | Initial Value | Description |
|-----------|----------------|-------------|
| `baseUrl` | `http://localhost:8080` | Base URL for API endpoints |
| `txId` | *empty* | Will be set dynamically during tests |

### 2. Start API Service

```bash
cd /path/to/rdbms
cargo run -p backend_service
```

### 3. Verify Service is Running

Send a simple GET request to confirm service is up:
- **URL**: `{{baseUrl}}/api/health`
- **Method**: GET
- **Expected**: `{"status":"healthy","version":"0.1.0"}`

---

## 🧪 Basic Functionality Tests

### Test 1: Health Check

**Purpose**: Verify API service is healthy and running

```http
GET {{baseUrl}}/api/health
```

**Headers**: None required  
**Body**: None required  

**Expected Response**:
```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

---

### Test 2: Create Table (DDL)

**Purpose**: Test table creation functionality

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "sql": "CREATE TABLE test_users (id INT PRIMARY KEY, name TEXT, email TEXT)"
}
```

**Expected Response**:
```json
{
  "columns": null,
  "rows": null,
  "rows_affected": null,
  "message": "OK"
}
```

---

### Test 3: Insert Data (Autocommit)

**Purpose**: Test data insertion with automatic transaction commit

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "sql": "INSERT INTO test_users VALUES (1, 'Alice', 'alice@example.com')"
}
```

**Expected Response**:
```json
{
  "columns": null,
  "rows": null,
  "rows_affected": 1,
  "message": "INSERT 0 1"
}
```

---

### Test 4: Select Data (Query)

**Purpose**: Test data retrieval

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "sql": "SELECT * FROM test_users"
}
```

**Expected Response**:
```json
{
  "columns": ["id", "name", "email"],
  "rows": [
    [
      {"type": "int", "value": 1},
      {"type": "text", "value": "Alice"},
      {"type": "text", "value": "alice@example.com"}
    ]
  ],
  "rows_affected": null,
  "message": null
}
```

---

### Test 5: Update Data

**Purpose**: Test data modification

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "sql": "UPDATE test_users SET email = 'alice@newdomain.com' WHERE id = 1"
}
```

**Expected Response**:
```json
{
  "columns": null,
  "rows": null,
  "rows_affected": 1,
  "message": "UPDATE 1"
}
```

---

### Test 6: Delete Data

**Purpose**: Test data deletion

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "sql": "DELETE FROM test_users WHERE id = 1"
}
```

**Expected Response**:
```json
{
  "columns": null,
  "rows": null,
  "rows_affected": 1,
  "message": "DELETE 1"
}
```

---

### Test 7: Drop Table

**Purpose**: Test table removal

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "sql": "DROP TABLE test_users"
}
```

**Expected Response**:
```json
{
  "columns": null,
  "rows": null,
  "rows_affected": null,
  "message": "OK"
}
```

---

## 🔄 Transaction Management Tests

### Test 8: Begin Transaction

**Purpose**: Test transaction initialization

```http
POST {{baseUrl}}/api/tx/begin
```

**Headers**:
```
Content-Type: application/json
```

**Body**: None required

**Expected Response**:
```json
{
  "tx_id": "uuid-string-here"
}
```

**Postman Test Script** (Tests tab):
```javascript
// Extract transaction ID for use in subsequent requests
const response = pm.response.json();
if (response.tx_id) {
    pm.environment.set("txId", response.tx_id);
    pm.test("Transaction ID generated", () => {
        pm.expect(response.tx_id).to.be.a('string');
        pm.expect(response.tx_id).to.match(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
    });
}
```

---

### Test 9: Insert Within Transaction

**Purpose**: Test data insertion within transaction context

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "sql": "INSERT INTO test_users VALUES (2, 'Bob', 'bob@example.com')",
  "tx_id": "{{txId}}"
}
```

**Expected Response**:
```json
{
  "columns": null,
  "rows": null,
  "rows_affected": 1,
  "message": "INSERT 0 1"
}
```

---

### Test 10: Select Within Transaction

**Purpose**: Test data visibility within transaction

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "sql": "SELECT * FROM test_users WHERE name = 'Bob'",
  "tx_id": "{{txId}}"
}
```

**Expected Response**:
```json
{
  "columns": ["id", "name", "email"],
  "rows": [
    [
      {"type": "int", "value": 2},
      {"type": "text", "value": "Bob"},
      {"type": "text", "value": "bob@example.com"}
    ]
  ],
  "rows_affected": null,
  "message": null
}
```

---

### Test 11: Commit Transaction

**Purpose**: Test transaction commit

```http
POST {{baseUrl}}/api/tx/{{txId}}/commit
```

**Headers**:
```
Content-Type: application/json
```

**Body**: None required

**Expected Response**:
```json
{
  "message": "Transaction committed"
}
```

---

### Test 12: Abort Transaction

**Purpose**: Test transaction rollback

**http
POST {{baseUrl}}/api/tx/{{txId}}/abort
```

**Headers**:
```
Content-Type: application/json
```

**Body**: None required

**Expected Response**:
```json
{
  "message": "Transaction aborted"
}
```

---

## ❌ Error Handling Tests

### Test 13: Invalid SQL Syntax

**Purpose**: Test SQL syntax error handling

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "sql": "INVALID SQL SYNTAX HERE"
}
```

**Expected Response** (HTTP 400):
```json
{
  "error_code": "SQL_PARSE_ERROR",
  "message": "sql parser error: Expected: an SQL statement, found: INVALID at Line: 1, Column: 1"
}
```

**Postman Test Script**:
```javascript
pm.test("SQL Parse Error Response", () => {
    const response = pm.response.json();
    pm.expect(response.error_code).to.eql("SQL_PARSE_ERROR");
    pm.expect(response.message).to.include("sql parser error");
});
```

---

### Test 14: Table Not Found

**Purpose**: Test catalog error handling

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "sql": "SELECT * FROM non_existent_table"
}
```

**Expected Response** (HTTP 400):
```json
{
  "error_code": "CATALOG_ERROR",
  "message": "table 'non_existent_table' not found"
}
```

---

### Test 15: Transaction Not Found

**Purpose**: Test invalid transaction ID handling

```http
POST {{baseUrl}}/api/tx/invalid-tx-id/commit
```

**Headers**:
```
Content-Type: application/json
```

**Body**: None required

**Expected Response** (HTTP 404):
```json
{
  "error_code": "TX_NOT_FOUND",
  "message": "Transaction invalid-tx-id not found"
}
```

---

### Test 16: Duplicate Primary Key

**Purpose**: Test constraint violation handling

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "sql": "INSERT INTO test_users VALUES (1, 'Duplicate', 'dup@example.com')"
}
```

**Expected Response** (HTTP 400):
```json
{
  "error_code": "CONSTRAINT_VIOLATION",
  "message": "duplicate key value violates unique constraint"
}
```

---

## ⚡ Concurrency Tests

### Test 17: Concurrent Table Creation

**Purpose**: Test concurrent request handling

**Setup**: Create 5 parallel requests in Postman using `sendRequest` or manually run multiple instances

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body** (vary for each request):
```json
{
  "sql": "CREATE TABLE concurrent_test{{index}} (id INT PRIMARY KEY, name TEXT)"
}
```

**Postman Test Script**:
```javascript
// Test 17: Concurrent Requests
const totalRequests = 5;
const baseUrl = pm.environment.get("baseUrl");
const promises = [];

for (let i = 1; i <= totalRequests; i++) {
    const promise = new Promise((resolve) => {
        pm.sendRequest({
            url: `${baseUrl}/api/sql`,
            method: 'POST',
            header: { 'Content-Type': 'application/json' },
            body: {
                mode: 'raw',
                raw: JSON.stringify({
                    sql: `CREATE TABLE concurrent_test${i} (id INT PRIMARY KEY, name TEXT)`
                })
            }
        }, (err, res) => {
            resolve({ index: i, error: err, response: res });
        });
    });
    promises.push(promise);
}

Promise.all(promises).then(results => {
    const successCount = results.filter(r => !r.error && r.response.code === 200).length;
    const errorCount = results.length - successCount;
    
    pm.test(`Concurrent Requests - ${successCount} success, ${errorCount} errors`, () => {
        pm.expect(successCount + errorCount).to.eql(totalRequests);
    });
});
```

---

### Test 18: Concurrent Inserts

**Purpose**: Test concurrent data insertion

```http
POST {{baseUrl}}/api/sql
```

**Headers**:
```
Content-Type: application/json
```

**Body** (for each concurrent request):
```json
{
  "sql": "INSERT INTO test_users VALUES ({{index}}, 'User{{index}}', 'user{{index}}@example.com')"
}
```

---

## 📦 Complete Test Collection

### Import the Full Collection

Copy the following JSON and import it into Postman:

```json
{
  "info": {
    "name": "RDBMS API Complete Test Suite",
    "description": "Comprehensive test suite for RDBMS REST API including basic functionality, transactions, error handling, and concurrency",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": [
    {
      "name": "01 - Health Check",
      "event": [
        {
          "listen": "test",
          "script": {
            "exec": [
              "pm.test(\"Status is healthy\", () => {",
              "    pm.expect(pm.response.json().status).to.eql(\"healthy\");",
              "});",
              "",
              "pm.test(\"Version is returned\", () => {",
              "    pm.expect(pm.response.json().version).to.be.a('string');",
              "});"
            ]
          }
        }
      ],
      "request": {
        "method": "GET",
        "header": [],
        "url": {
          "raw": "{{baseUrl}}/api/health",
          "host": ["{{baseUrl}}"],
          "path": ["api", "health"]
        }
      }
    },
    {
      "name": "02 - Create Table",
      "event": [
        {
          "listen": "test",
          "script": {
            "exec": [
              "pm.test(\"Table created successfully\", () => {",
              "    const response = pm.response.json();",
              "    pm.expect(response.message).to.eql(\"OK\");",
              "});"
            ]
          }
        }
      ],
      "request": {
        "method": "POST",
        "header": [
          {
            "key": "Content-Type",
            "value": "application/json"
          }
        ],
        "body": {
          "mode": "raw",
          "raw": "{\n  \"sql\": \"CREATE TABLE test_users (id INT PRIMARY KEY, name TEXT, email TEXT)\"\n}"
        },
        "url": {
          "raw": "{{baseUrl}}/api/sql",
          "host": ["{{baseUrl}}"],
          "path": ["api", "sql"]
        }
      }
    },
    {
      "name": "03 - Insert Data (Autocommit)",
      "event": [
        {
          "listen": "test",
          "script": {
            "exec": [
              "pm.test(\"Insert successful\", () => {",
              "    const response = pm.response.json();",
              "    pm.expect(response.rows_affected).to.eql(1);",
              "    pm.expect(response.message).to.include(\"INSERT\");",
              "});"
            ]
          }
        }
      ],
      "request": {
        "method": "POST",
        "header": [
          {
            "key": "Content-Type",
            "value": "application/json"
          }
        ],
        "body": {
          "mode": "raw",
          "raw": "{\n  \"sql\": \"INSERT INTO test_users VALUES (1, 'Alice', 'alice@example.com')\"\n}"
        },
        "url": {
          "raw": "{{baseUrl}}/api/sql",
          "host": ["{{baseUrl}}"],
          "path": ["api", "sql"]
        }
      }
    },
    {
      "name": "04 - Select Data",
      "event": [
        {
          "listen": "test",
          "script": {
            "exec": [
              "pm.test(\"Query returns data\", () => {",
              "    const response = pm.response.json();",
              "    pm.expect(response.columns).to.be.an('array');",
              "    pm.expect(response.rows).to.be.an('array');",
              "    pm.expect(response.rows.length).to.be.gte(0);",
              "});"
            ]
          }
        }
      ],
      "request": {
        "method": "POST",
        "header": [
          {
            "key": "Content-Type",
            "value": "application/json"
          }
        ],
        "body": {
          "mode": "raw",
          "raw": "{\n  \"sql\": \"SELECT * FROM test_users\"\n}"
        },
        "url": {
          "raw": "{{baseUrl}}/api/sql",
          "host": ["{{baseUrl}}"],
          "path": ["api", "sql"]
        }
      }
    },
    {
      "name": "05 - Begin Transaction",
      "event": [
        {
          "listen": "test",
          "script": {
            "exec": [
              "const response = pm.response.json();",
              "if (response.tx_id) {",
              "    pm.environment.set(\"txId\", response.tx_id);",
              "    pm.test(\"Transaction ID generated\", () => {",
              "        pm.expect(response.tx_id).to.be.a('string');",
              "        pm.expect(response.tx_id).to.match(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);",
              "    });",
              "}"
            ]
          }
        }
      ],
      "request": {
        "method": "POST",
        "header": [
          {
            "key": "Content-Type",
            "value": "application/json"
          }
        ],
        "url": {
          "raw": "{{baseUrl}}/api/tx/begin",
          "host": ["{{baseUrl}}"],
          "path": ["api", "tx", "begin"]
        }
      }
    },
    {
      "name": "06 - Insert in Transaction",
      "event": [
        {
          "listen": "test",
          "script": {
            "exec": [
              "pm.test(\"Insert in transaction successful\", () => {",
              "    const response = pm.response.json();",
              "    pm.expect(response.rows_affected).to.eql(1);",
              "    pm.expect(response.message).to.include(\"INSERT\");",
              "});"
            ]
          }
        }
      ],
      "request": {
        "method": "POST",
        "header": [
          {
            "key": "Content-Type",
            "value": "application/json"
          }
        ],
        "body": {
          "mode": "raw",
          "raw": "{\n  \"sql\": \"INSERT INTO test_users VALUES (2, 'Bob', 'bob@example.com')\",\n  \"tx_id\": \"{{txId}}\"\n}"
        },
        "url": {
          "raw": "{{baseUrl}}/api/sql",
          "host": ["{{baseUrl}}"],
          "path": ["api", "sql"]
        }
      }
    },
    {
      "name": "07 - Commit Transaction",
      "event": [
        {
          "listen": "test",
          "script": {
            "exec": [
              "pm.test(\"Transaction committed\", () => {",
              "    const response = pm.response.json();",
              "    pm.expect(response.message).to.eql(\"Transaction committed\");",
              "});"
            ]
          }
        }
      ],
      "request": {
        "method": "POST",
        "header": [
          {
            "key": "Content-Type",
            "value": "application/json"
          }
        ],
        "url": {
          "raw": "{{baseUrl}}/api/tx/{{txId}}/commit",
          "host": ["{{baseUrl}}"],
          "path": ["api", "tx", "{{txId}}", "commit"]
        }
      }
    },
    {
      "name": "08 - Invalid SQL Error",
      "event": [
        {
          "listen": "test",
          "script": {
            "exec": [
              "pm.test(\"SQL Parse Error Response\", () => {",
              "    const response = pm.response.json();",
              "    pm.expect(response.error_code).to.eql(\"SQL_PARSE_ERROR\");",
              "    pm.expect(response.message).to.include(\"sql parser error\");",
              "});"
            ]
          }
        }
      ],
      "request": {
        "method": "POST",
        "header": [
          {
            "key": "Content-Type",
            "value": "application/json"
          }
        ],
        "body": {
          "mode": "raw",
          "raw": "{\n  \"sql\": \"INVALID SQL SYNTAX HERE\"\n}"
        },
        "url": {
          "raw": "{{baseUrl}}/api/sql",
          "host": ["{{baseUrl}}"],
          "path": ["api", "sql"]
        }
      }
    },
    {
      "name": "09 - Transaction Not Found",
      "event": [
        {
          "listen": "test",
          "script": {
            "exec": [
              "pm.test(\"Transaction not found error\", () => {",
              "    const response = pm.response.json();",
              "    pm.expect(response.error_code).to.eql(\"TX_NOT_FOUND\");",
              "    pm.expect(response.message).to.include(\"not found\");",
              "});"
            ]
          }
        }
      ],
      "request": {
        "method": "POST",
        "header": [
          {
            "key": "Content-Type",
            "value": "application/json"
          }
        ],
        "url": {
          "raw": "{{baseUrl}}/api/tx/invalid-transaction-id/commit",
          "host": ["{{baseUrl}}"],
          "path": ["api", "tx", "invalid-transaction-id", "commit"]
        }
      }
    },
    {
      "name": "10 - Drop Table",
      "event": [
        {
          "listen": "test",
          "script": {
            "exec": [
              "pm.test(\"Table dropped successfully\", () => {",
              "    const response = pm.response.json();",
              "    pm.expect(response.message).to.eql(\"OK\");",
              "});"
            ]
          }
        }
      ],
      "request": {
        "method": "POST",
        "header": [
          {
            "key": "Content-Type",
            "value": "application/json"
          }
        ],
        "body": {
          "mode": "raw",
          "raw": "{\n  \"sql\": \"DROP TABLE test_users\"\n}"
        },
        "url": {
          "raw": "{{baseUrl}}/api/sql",
          "host": ["{{baseUrl}}"],
          "path": ["api", "sql"]
        }
      }
    }
  ],
  "variable": [
    {
      "key": "baseUrl",
      "value": "http://localhost:8080"
    },
    {
      "key": "txId",
      "value": ""
    }
  ]
}
```

---

## 📊 Expected Results Summary

### Successful Test Indicators

| Test | Success Criteria | HTTP Status |
|------|------------------|-------------|
| Health Check | `"status": "healthy"` | 200 |
| Create Table | `"message": "OK"` | 200 |
| Insert Data | `"rows_affected": 1` | 200 |
| Select Data | `"columns": [...]` and `"rows": [...]` | 200 |
| Update Data | `"rows_affected": 1` | 200 |
| Delete Data | `"rows_affected": 1` | 200 |
| Drop Table | `"message": "OK"` | 200 |
| Begin Transaction | `"tx_id": "uuid"` | 200 |
| Commit Transaction | `"message": "Transaction committed"` | 200 |
| Abort Transaction | `"message": "Transaction aborted"` | 200 |

### Error Response Indicators

| Test | Error Criteria | HTTP Status |
|------|----------------|-------------|
| Invalid SQL | `"error_code": "SQL_PARSE_ERROR"` | 400 |
| Table Not Found | `"error_code": "CATALOG_ERROR"` | 400 |
| Constraint Violation | `"error_code": "CONSTRAINT_VIOLATION"` | 400 |
| Transaction Not Found | `"error_code": "TX_NOT_FOUND"` | 400 |
| Internal Error | `"error_code": "INTERNAL_ERROR"` | 500 |

---

## 🔧 Troubleshooting

### Common Issues

1. **Service Not Running**
   ```
   Error: connect ECONNREFUSED 127.0.0.1:8080
   ```
   **Solution**: Start the API service with `cargo run -p api`

2. **Invalid JSON in Request**
   ```
   Error: "message": "expected value at line 1 column 2"
   ```
   **Solution**: Check JSON syntax in request body

3. **Transaction ID Not Set**
   ```
   Response: {"error_code":"TX_NOT_FOUND","message":"Transaction  not found"}
   ```
   **Solution**: Run "Begin Transaction" test first to set txId environment variable

4. **Table Already Exists**
   ```
   Response: {"error_code":"CATALOG_ERROR","message":"table 'test_users' already exists"}
   ```
   **Solution**: Drop the table first or use a different table name

### Running Tests in Order

1. Run "01 - Health Check" to verify service is up
2. Run "02 - Create Table" to set up test data
3. Run "03 - Insert Data" to test autocommit
4. Run "04 - Select Data" to verify data was inserted
5. Run "05 - Begin Transaction" to start a new transaction
6. Run "06 - Insert in Transaction" to test transaction context
7. Run "07 - Commit Transaction" to test transaction completion
8. Run error handling tests (08-09) with invalid inputs
9. Run "10 - Drop Table" to clean up

### Performance Testing

For load testing, use Postman Runner or Newman:

```bash
# Install Newman
npm install -g newman

# Run collection
newman run rdbms-api-collection.json -e environment.json
```

---

## 📝 Test Report Template

Use this template to track your test results:

```
=== RDBMS API Test Results ===
Date: ___________
Environment: Development/Staging/Production
Base URL: http://localhost:8080

✅ PASSED TESTS:
[ ] Health Check
[ ] Create Table
[ ] Insert Data (Autocommit)
[ ] Select Data
[ ] Update Data
[ ] Delete Data
[ ] Begin Transaction
[ ] Insert in Transaction
[ ] Commit Transaction
[ ] Abort Transaction

❌ FAILED TESTS:
[ ] Invalid SQL Error Handling
[ ] Table Not Found Error
[ ] Constraint Violation Error
[ ] Transaction Not Found Error
[ ] Concurrent Request Handling

⚠️  ISSUES FOUND:
____________________________________________________________________
____________________________________________________________________

🎯 OVERALL STATUS: ___% Tests Passed
```

---

## 📚 Additional Resources

- [API Documentation](./run.md#api-reference)
- [Error Codes Reference](./run.md#error-codes)
- [Environment Setup](./run.md#setup)
- [Integration Examples](./run.md#integration-examples)

---

*Last Updated: January 2026*
*API Version: v0.1.0*