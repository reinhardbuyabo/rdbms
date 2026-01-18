# RDBMS Quickstart Guide

A lightweight embedded RDBMS with full ACID transaction support, written in Rust.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Building from Source](#building-from-source)
3. [Using the REPL](#using-the-repl)
4. [Using the Backend Service](#using-the-backend-service)
5. [Using the TCP Server](#using-the-tcp-server)
6. [Running with Docker](#running-with-docker)
7. [Example CRUD Operations](#example-crud-operations)
8. [Transaction Demo](#transaction-demo)

---

## Quick Start

### Option 1: Download Release (Coming Soon)

```bash
# Download the latest release for your platform
curl -L https://github.com/reinhardbuyabo/rdbms/releases/latest/download/rdbms-v0.4.0-x86_64-unknown-linux-gnu.tar.gz
tar -xzf rdbms-v0.4.0-x86_64-unknown-linux-gnu.tar.gz
./rdbms --help
```

### Option 2: Build from Source

```bash
git clone https://github.com/reinhardbuyabo/rdbms.git
cd rdbms
make build-release
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

# Build all components (REPL, TCP server, and backend-service)
make build-release

# Verify binaries
ls -la target/release/rdbms            # REPL binary
ls -la target/release/rdbmsd           # TCP server binary
ls -la target/release/backend-service  # REST API service binary
```

**Alternative (using Cargo directly):**
```bash
cargo build --release                    # Build all
cargo build --release -p db              # Build REPL and TCP server
cargo build --release -p backend-service # Build REST API service
```

**Build Output:**
- `target/release/rdbms` - CLI REPL (interactive terminal)
- `target/release/rdbmsd` - TCP Server (systemd-managed, for backend services)
- `target/release/backend-service` - REST API Service (for frontend consumption)

---

## Using the REPL

The REPL provides an interactive command-line interface to the database.

### Start the REPL

```bash
# Using make with default database (./data.db)
make run-repl

# Using make with custom database path
make run-repl DB_PATH=/path/to/your/database.db

# Alternative: Direct binary usage
./target/release/rdbms --db /path/to/database.db
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
$ make run-repl DB_PATH=/tmp/mydb.db
RDBMS REPL v0.4.0
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

## Using the Backend Service

The backend-service provides REST API endpoints for SQL execution and transaction management. This is the recommended interface for frontend applications.

### Start the Backend Service

```bash
# Using make with defaults (port 8080, database: ./data.db)
make run-backend-service

# Using make with custom settings
make run-backend-service PORT=3000 DB_PATH=/tmp/mydb.db

# Alternative: Using the release binary directly
DB_PATH=/tmp/mydb.db PORT=3000 ./target/release/backend-service
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
| `BIND` | `0.0.0.0` | Bind address |

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

## Using the TCP Server

The TCP server (`rdbmsd`) is a lightweight protocol server suitable for:
- Systemd service integration
- Backend services connecting programmatically
- High-performance scenarios

### Start the TCP Server

```bash
# Using make with defaults (port 5432, database: ./data.db)
make run-server

# Using make with custom settings
make run-server SERVER_PORT=5432 DB_PATH=/tmp/mydb.db

# Alternative: Using the release binary directly
./target/release/rdbmsd --db /tmp/mydb.db --listen 0.0.0.0:5432
```

### Protocol (JSON-RPC style)

**Request format:**
```json
{"method": "execute", "params": ["SQL_STATEMENT"]}
{"method": "ping"}
```

**Response format:**
```json
{"status": "ok", "result": {...}, "error": null}
{"status": "error", "result": null, "error": "error message"}
```

### Example with netcat

```bash
# Ping
echo '{"method":"ping"}' | nc -w 2 localhost 5432

# Execute SQL
echo '{"method":"execute","params":["CREATE TABLE test (id INT, val INT);"]}' | nc -w 2 localhost 5432

# Query data
echo '{"method":"execute","params":["SELECT * FROM test;"]}' | nc -w 2 localhost 5432
```

### Systemd Integration

For production deployment, install as a systemd service:

```bash
# Install (requires root)
sudo make install-systemd

# Enable on boot
sudo systemctl enable rdbms

# Start the service
sudo systemctl start rdbms

# Check status
sudo systemctl status rdbms

# View logs
journalctl -u rdbms -f
```

---

## Running with Docker

### Build the Docker Image

```bash
# Build using make
make docker-build

# Or using docker directly
docker build -t rdbms:latest .
```

### Run the Container

#### Run TCP Server

```bash
# Run using make
make docker-run

# Or using docker directly
docker run -d \
  --name rdbms \
  -p 5432:5432 \
  -v $(pwd)/data:/data \
  rdbms:latest

# View logs
docker logs -f rdbms

# Stop
make docker-stop
# Or: docker stop rdbms && docker rm rdbms
```

#### Run Backend Service via Docker

```bash
docker run -d \
  --name rdbms-api \
  -p 8080:8080 \
  -v $(pwd)/data:/data \
  -e SERVICE=api \
  rdbms:latest
```

#### Run REPL Interactively

```bash
docker run -it --rm \
  -v $(pwd)/data:/data \
  rdbms:latest \
  ./target/release/rdbms --db /data/mydb.db
```

### Docker Compose

A `docker-compose.yml` is included in the repository for orchestration:

```bash
# Start all services
docker compose up -d

# View logs
docker compose logs -f

# Stop all services
docker compose down

# Stop and remove data volumes
docker compose down -v
```

**Services:**
| Service | Port | Description |
|---------|------|-------------|
| `rdbms-server` | 5432 | TCP server |
| `backend-service` | 8080 | REST API |
| `db-init` | - | Database initialization |

**Initialize database with schema and seed data:**
```bash
# First time setup
docker compose down -v
docker compose up -d

# The db-init service runs automatically on first start
# To re-initialize: docker compose up -d db-init
```

**API Access:**
```bash
# Health check
curl http://localhost:8080/api/health

# Execute SQL
curl -X POST http://localhost:8080/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"CREATE TABLE test (id INT, name TEXT)"}'
```

**Note:** The `db-init` service uses `python3 /usr/local/bin/docker-init.py` (a Python-based initializer) to execute SQL files from the `db/` directory on startup.

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

### Using curl (Backend Service)

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

### Using netcat (TCP Server)

```bash
# Create table
echo '{"method":"execute","params":["CREATE TABLE employees (id INT PRIMARY KEY, name TEXT, department TEXT, salary INT)"]}' | nc -w 2 localhost 5432

# Insert employees
echo '{"method":"execute","params":["INSERT INTO employees VALUES (1, '\''Alice'\'', '\''Engineering'\'', 90000)"]}' | nc -w 2 localhost 5432

# Query all
echo '{"method":"execute","params":["SELECT * FROM employees"]}' | nc -w 2 localhost 5432
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

### Transaction with curl (Backend Service)

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

### Testing Transaction ABORT

The following demonstrates that ABORT correctly rolls back changes:

```bash
BASE_URL="http://localhost:8080/api"

# Setup fresh table
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"CREATE TABLE test_abort (id INT PRIMARY KEY, val INT)"}'
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"INSERT INTO test_abort VALUES (1, 100)"}'

# Initial state: val=100
echo "Initial state (expect val=100):"
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"SELECT * FROM test_abort"}'

# Begin transaction and update to 200
TX_RESPONSE=$(curl -s -X POST $BASE_URL/tx/begin -H "Content-Type: application/json")
TX_ID=$(echo $TX_RESPONSE | grep -o '"tx_id":"[^"]*"' | cut -d'"' -f4)

curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d "{\"sql\":\"UPDATE test_abort SET val = 200 WHERE id = 1\",\"tx_id\":\"$TX_ID\"}"

# Within transaction: val=200
echo "Within transaction (expect val=200):"
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d "{\"sql\":\"SELECT * FROM test_abort\",\"tx_id\":\"$TX_ID\"}"

# ABORT the transaction
echo "Aborting transaction..."
curl -X POST "$BASE_URL/tx/$TX_ID/abort" \
  -H "Content-Type: application/json"

# After abort: val=100 (correctly rolled back!)
echo "After abort (expect val=100 - ROLLED BACK):"
curl -X POST $BASE_URL/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"SELECT * FROM test_abort"}'
```

**Expected Output:**
```
Initial state: [{"type":"int","value":1},{"type":"int","value":100}]
Within transaction: [{"type":"int","value":1},{"type":"int","value":200}]
After abort: [{"type":"int","value":1},{"type":"int","value":100}]  ✓ Atomicity preserved!
```

---

## Quick Reference

### Make Commands

| Command | Description |
|---------|-------------|
| `make build-release` | Build optimized release binaries |
| `make run-repl` | Start REPL (use `DB_PATH=...`) |
| `make run-server` | Start TCP server on port 5432 (use `SERVER_PORT=... DB_PATH=...`) |
| `make run-backend-service` | Start REST API on port 8080 (use `PORT=... DB_PATH=...`) |
| `make docker-build` | Build Docker image |
| `make docker-run` | Run TCP server in Docker |
| `make docker-stop` | Stop Docker containers |
| `make install-systemd` | Install as systemd service (requires root) |
| `make test` | Run all tests |

### Binaries

| Binary | Purpose | Interface |
|--------|---------|-----------|
| `rdbms` | Interactive REPL | Terminal |
| `rdbmsd` | TCP Server | JSON-RPC over TCP (port 5432) |
| `backend-service` | REST API | HTTP (port 8080) |

### Examples

```bash
# Build and run REPL with custom database
make build-release && make run-repl DB_PATH=/tmp/myapp.db

# Build and run TCP server
make build-release && make run-server SERVER_PORT=5432 DB_PATH=/tmp/myapp.db

# Build and run REST API service
make build-release && make run-backend-service PORT=3000 DB_PATH=/tmp/myapp.db

# Build Docker image and run
make docker-build && make docker-run
```

---

## Next Steps

- See [run.md](run.md) for detailed documentation
- See [postman-testing-guide.md](postman-testing-guide.md) for Postman collection
- Run tests: `make test`
- Install as systemd service: `sudo make install-systemd`
- Report issues: https://github.com/reinhardbuyabo/rdbms/issues

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| v0.4.0 | Jan 2026 | Transaction APIs, Catalog persistence, REST API, ORDER BY support |
| v0.3.0 | Jan 2026 | ACID transactions, Lock Manager, WAL, Schema persistence |
| v0.2.0 | Jan 2026 | Query engine, B+Tree indexes |
| v0.1.0 | Jan 2026 | Initial release, basic CRUD |
