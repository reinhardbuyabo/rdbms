# RDBMS Quickstart Guide

A lightweight embedded RDBMS with full ACID transaction support, written in Rust.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Building from Source](#building-from-source)
3. [Using the REPL](#using-the-repl)
4. [Using the REST API](#using-the-rest-api)
5. [Running with Docker](#running-with-docker)
6. [Example CRUD Operations](#example-crud-operations)
7. [Transaction Demo](#transaction-demo)

---

## Quick Start

### Option 1: Download Release (Coming Soon)

```bash
# Download the latest release for your platform
curl -L https://github.com/reinhardbuyabo/rdbms/releases/latest/download/rdbms-v0.3.0-x86_64-unknown-linux-gnu.tar.gz
tar -xzf rdbms-v0.3.0-x86_64-unknown-linux-gnu.tar.gz
./rdbms --help
```

### Option 2: Build from Source

```bash
git clone https://github.com/reinhardbuyabo/rdbms.git
cd rdbms
cargo build --release
```

---

## Building from Source

### Prerequisites

- **Rust** 1.70 or later
- **Cargo** (comes with Rust)
- **Git**

### Build Commands

```bash
# Clone the repository
git clone https://github.com/reinhardbuyabo/rdbms.git
cd rdbms

# Build all components
cargo build --release

# Build only the REPL
cargo build --release -p db

# Build only the API service
cargo build --release -p api

# Verify binaries
ls -la target/release/rdbms    # REPL binary
ls -la target/release/rdbmsd   # Server binary
ls -la target/release/api      # API service binary
```

**Build Output:**
- `target/release/rdbms` - CLI REPL (7.6 MB)
- `target/release/rdbmsd` - TCP Server (7.6 MB)
- `target/release/api` - REST API Service

---

## Using the REPL

The REPL provides an interactive command-line interface to the database.

### Start the REPL

```bash
# Using default database (./data.db)
./target/release/rdbms

# Using custom database path
./target/release/rdbms --db /path/to/your/database.db
```

### REPL Commands

```sql
-- Create a table
CREATE TABLE users (id INT PRIMARY KEY, name TEXT, email TEXT);

-- Insert data
INSERT INTO users VALUES (1, 'Alice', 'alice@example.com');
INSERT INTO users VALUES (2, 'Bob', 'bob@example.com');

-- Query data
SELECT * FROM users;
SELECT * FROM users WHERE id = 1;

-- Update data
UPDATE users SET email = 'alice@new.com' WHERE id = 1;

-- Delete data
DELETE FROM users WHERE id = 2;

-- Drop table
DROP TABLE users;

-- Exit REPL
.exit
```

### Interactive Example

```bash
$ ./target/release/rdbms --db /tmp/mydb.db
RDBMS REPL v0.3.0
Using database file: /tmp/mydb.db

> CREATE TABLE users (id INT PRIMARY KEY, name TEXT);
OK
> INSERT INTO users VALUES (1, 'Alice');
INSERT 0 1
> INSERT INTO users VALUES (2, 'Bob');
INSERT 0 1
> SELECT * FROM users;
+---+-------+
| * |       |
+===============================+
| 1 | Alice |
|---+-------+
| 2 | Bob   |
+---+-------+
(2 rows)
> .exit
```

---

## Using the REST API

The REST API provides HTTP endpoints for SQL execution and transaction management.

### Start the API Server

```bash
# With defaults (port 8080, database: ./data.db)
cargo run -p api --release

# With custom settings
DB_PATH=/tmp/mydb.db PORT=3000 cargo run -p api --release

# Using pre-built binary
DB_PATH=/tmp/mydb.db PORT=3000 ./target/release/api
```

### API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/health` | Health check |
| `POST` | `/api/sql` | Execute SQL |
| `POST` | `/api/tx/begin` | Begin transaction |
| `POST` | `/api/tx/{id}/commit` | Commit transaction |
| `POST` | `/api/tx/{id}/abort` | Abort transaction |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8080` | HTTP server port |
| `DB_PATH` | `./data.db` | Database file path |

### curl Examples

#### 1. Health Check

```bash
curl http://localhost:8080/api/health
# Response: {"status":"healthy","version":"0.2.0"}
```

#### 2. Execute SQL (Autocommit)

```bash
# Create table
curl -X POST http://localhost:8080/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"CREATE TABLE products (id INT PRIMARY KEY, name TEXT, price INT)"}'

# Insert data (note: use single quotes for text values in bash)
curl -X POST http://localhost:8080/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"INSERT INTO products VALUES (1, '\''Widget'\'', 100)"}'

# Query data
curl -X POST http://localhost:8080/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"SELECT * FROM products"}'
```

#### 3. Transaction Workflow

```bash
# Begin transaction
TX_RESPONSE=$(curl -s -X POST http://localhost:8080/api/tx/begin \
  -H "Content-Type: application/json")
TX_ID=$(echo $TX_RESPONSE | grep -o '"tx_id":"[^"]*"' | cut -d'"' -f4)
echo "Transaction ID: $TX_ID"

# Execute within transaction
curl -X POST http://localhost:8080/api/sql \
  -H "Content-Type: application/json" \
  -d "{\"sql\":\"INSERT INTO products VALUES (2, 'Gadget', 250)\",\"tx_id\":\"$TX_ID\"}"

# Commit transaction
curl -X POST "http://localhost:8080/api/tx/$TX_ID/commit" \
  -H "Content-Type: application/json"

# Abort transaction (rolls back changes)
curl -X POST "http://localhost:8080/api/tx/$TX_ID/abort" \
  -H "Content-Type: application/json"
```

---

## Running with Docker

### Build the Docker Image

```bash
# Build the image
docker build -t rdbms:latest .

# Build with specific tag
docker build -t rdbms:v0.3.0 .
```

### Run the Container

#### Run API Service

```bash
# Run in detached mode
docker run -d \
  --name rdbms \
  -p 8080:8080 \
  -v $(pwd)/data:/data \
  rdbms:latest

# View logs
docker logs -f rdbms

# Stop
docker stop rdbms
docker rm rdbms
```

#### Run REPL Interactively

```bash
docker run -it --rm \
  -v $(pwd)/data:/data \
  rdbms:latest \
  ./target/release/rdbms --db /data/mydb.db
```

### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  rdbms:
    image: rdbms:latest
    container_name: rdbms
    ports:
      - "8080:8080"
    volumes:
      - ./data:/data
    restart: unless-stopped
```

Start the service:

```bash
docker-compose up -d
docker-compose logs -f rdbms
docker-compose down
```

---

## Example CRUD Operations

### Using the REPL

```sql
-- Create
CREATE TABLE employees (
  id INT PRIMARY KEY,
  name TEXT,
  department TEXT,
  salary INT
);

-- Read
INSERT INTO employees VALUES (1, 'Alice', 'Engineering', 90000);
INSERT INTO employees VALUES (2, 'Bob', 'Sales', 75000);
INSERT INTO employees VALUES (3, 'Carol', 'Engineering', 95000);

-- Query
SELECT * FROM employees;
SELECT * FROM employees WHERE department = 'Engineering';
SELECT name, salary FROM employees WHERE salary > 80000;

-- Update
UPDATE employees SET salary = 100000 WHERE id = 3;

-- Delete
DELETE FROM employees WHERE id = 2;

-- Verify
SELECT * FROM employees;
```

### Using curl

```bash
BASE_URL="http://localhost:8080/api"

# Create table
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"CREATE TABLE employees (id INT PRIMARY KEY, name TEXT, department TEXT, salary INT)"}'

# Insert employees
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"INSERT INTO employees VALUES (1, '\''Alice'\'', '\''Engineering'\'', 90000)"}'
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"INSERT INTO employees VALUES (2, '\''Bob'\'', '\''Sales'\'', 75000)"}'
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"INSERT INTO employees VALUES (3, '\''Carol'\'', '\''Engineering'\'', 95000)"}'

# Query all
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"SELECT * FROM employees"}'

# Query with filter
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"SELECT name, salary FROM employees WHERE salary > 80000"}'

# Update
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"UPDATE employees SET salary = 100000 WHERE id = 3"}'

# Delete
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"DELETE FROM employees WHERE id = 2"}'
```

---

## Transaction Demo

### ACID Properties

RDBMS ensures **ACID** properties for transactions:

- **Atomicity**: All changes either commit or abort together
- **Consistency**: Database moves from one valid state to another
- **Isolation**: Concurrent transactions don't interfere
- **Durability**: Committed changes survive crashes

### Interactive Transaction Demo (REPL)

```sql
> CREATE TABLE accounts (id INT PRIMARY KEY, balance INT);
OK
> INSERT INTO accounts VALUES (1, 1000);
INSERT 0 1
> INSERT INTO accounts VALUES (2, 500);
INSERT 0 1

-- Start transaction (BEGIN is implicit in REPL)
> UPDATE accounts SET balance = balance - 100 WHERE id = 1;
UPDATE 1
> UPDATE accounts SET balance = balance + 100 WHERE id = 2;
UPDATE 1

-- Verify before commit
> SELECT * FROM accounts;
+---+---------+
| * |         |
+===============================+
| 1 | 900     |
|---+---------+
| 2 | 600     |
+---+---------+
(2 rows)

-- Commit is automatic in REPL mode
```

### Transaction with curl

```bash
BASE_URL="http://localhost:8080/api"

# Setup
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"CREATE TABLE accounts (id INT PRIMARY KEY, balance INT)"}'
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"INSERT INTO accounts VALUES (1, 1000)"}'
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"INSERT INTO accounts VALUES (2, 500)"}'

# Begin transaction
TX_RESPONSE=$(curl -s -X POST $BASE_URL/tx/begin -H "Content-Type: application/json")
TX_ID=$(echo $TX_RESPONSE | grep -o '"tx_id":"[^"]*"' | cut -d'"' -f4)
echo "Started transaction: $TX_ID"

# Transfer funds within transaction
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d "{\"sql\":\"UPDATE accounts SET balance = balance - 100 WHERE id = 1\",\"tx_id\":\"$TX_ID\"}"

curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d "{\"sql\":\"UPDATE accounts SET balance = balance + 100 WHERE id = 2\",\"tx_id\":\"$TX_ID\"}"

# Check balance within transaction
echo "Balance within transaction:"
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d "{\"sql\":\"SELECT * FROM accounts\",\"tx_id\":\"$TX_ID\"}"

# Commit the transaction
echo "Committing transaction..."
curl -X POST "$BASE_URL/tx/$TX_ID/commit" \
  -H "Content-Type: application/json"

# Verify after commit
echo "Balance after commit:"
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"SELECT * FROM accounts"}'
```

---

## Next Steps

- See [run.md](run.md) for detailed documentation
- See [postman-testing-guide.md](postman-testing-guide.md) for Postman collection
- Run tests: `cargo test`
- Report issues: https://github.com/reinhardbuyabo/rdbms/issues

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| v0.3.0 | Jan 2026 | ACID transactions, Lock Manager, WAL, Schema persistence |
| v0.2.0 | Jan 2026 | Query engine, B+Tree indexes |
| v0.1.0 | Jan 2026 | Initial release, basic CRUD |
