# RDBMS - A Rust Transactional Database

A lightweight, embedded RDBMS written in Rust with full ACID transaction support, B+Tree indexes, and Write-Ahead Logging.

![CI](https://github.com/reinhardbuyabo/rdbms/workflows/CI/badge.svg)
![License](https://img.shields.io/github/license/reinhardbuyabo/rdbms)
![Version](https://img.shields.io/github/v/release/reinhardbuyabo/rdbms)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/reinhardbuyabo/rdbms?utm_source=oss&utm_medium=github&utm_campaign=reinhardbuyabo%2Frdbms&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)

## Features

- **ACID Transactions**: Full atomicity, consistency, isolation, and durability
- **Write-Ahead Logging (WAL)**: Crash recovery with redo/undo
- **Lock Manager**: Two-phase locking with deadlock detection
- **B+Tree Indexes**: Composite indexes with range scan support
- **SQL Parser**: Basic SQL support (SELECT, INSERT, UPDATE, DELETE)
- **Blob Storage**: Large object support
- **Multiple Access Modes**:
  - REPL interactive mode (`rdbms`)
  - TCP server mode (`rdbmsd`) - JSON-RPC over TCP
  - REST API (`backend-service`) - HTTP API for frontend integration

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Application                               │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   REPL CLI  │  │  TCP Server │  │    REST API Service     │  │
│  │   (rdbms)   │  │   (rdbmsd)  │  │  (backend-service)      │  │
│  └──────┬──────┘  └──────┬──────┘  └───────────┬─────────────┘  │
├─────────┼────────────────┼─────────────────────┼─────────────────┤
│         │                │                     │                 │
│         ▼                ▼                     ▼                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                         Engine                             │  │
│  ├───────────────────────────────────────────────────────────┤  │
│  │  ┌───────────┐  ┌───────────┐  ┌─────────────────────────┐│  │
│  │  │  Catalog  │  │   Lock    │  │   TransactionManager    ││  │
│  │  │           │  │  Manager  │  │                         ││  │
│  │  └───────────┘  └───────────┘  └─────────────────────────┘│  │
│  │  ┌───────────┐  ┌───────────┐  ┌─────────────────────────┐│  │
│  │  │   Query   │  │  Recovery │  │    BufferPoolManager    ││  │
│  │  │   Engine  │  │  Manager  │  │                         ││  │
│  │  └───────────┘  └───────────┘  └─────────────────────────┘│  │
│  └───────────────────────────────────────────────────────────┘  │
│         │                │                     │                 │
│         ▼                ▼                     ▼                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                Storage (Disk + Buffer Pool)                │  │
│  ├───────────────────────────────────────────────────────────┤  │
│  │  ┌─────────────┐  ┌─────────────────────────────────────┐ │  │
│  │  │ Page Format │  │         B+Tree Index Pages          │ │  │
│  │  └─────────────┘  └─────────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                  │
│                           ▼                                      │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                Write-Ahead Log (WAL)                       │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

- Rust 1.70 or later
- Cargo

### Building from Source

```bash
# Clone the repository
git clone https://github.com/reinhardbuyabo/rdbms.git
cd rdbms

# Build all binaries
cargo build --release

# Run the REPL
./target/release/rdbms --db ./mydb

# Run as TCP server (default port 5432)
./target/release/rdbmsd --db ./mydb --listen 0.0.0.0:5432

# Run as REST API server (default port 8080)
./target/release/backend-service --db ./mydb --port 8080
```

### TCP Server API

The RDBMS TCP server accepts JSON-RPC style requests:

```json
// Request format
{"method": "execute", "params": ["SQL_STATEMENT"]}

// Response format (success)
{"status": "ok", "result": {...}, "error": null}

// Response format (error)
{"status": "error", "result": null, "error": "error message"}
```

**Example with Python:**

```python
import socket
import json

def execute(sql):
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(5)
    sock.connect(('127.0.0.1', 5432))
    request = {"method": "execute", "params": [sql]}
    sock.sendall(json.dumps(request).encode())
    sock.shutdown(socket.SHUT_WR)
    response = sock.recv(16384)
    sock.close()
    return json.loads(response.decode())

# Usage examples
execute("CREATE TABLE users (id INT PRIMARY KEY, name TEXT)")
execute("INSERT INTO users VALUES (1, 'Alice')")
execute("SELECT * FROM users")
```

**Example with netcat:**

```bash
# Ping
echo '{"method":"ping"}' | nc -w 2 127.0.0.1 5432

# Execute SQL
echo '{"method":"execute","params":["SELECT * FROM users"]}' | nc -w 2 127.0.0.1 5432
```

### REST API (backend-service)

The backend-service provides a REST API for integration with frontend applications:

```bash
# Start the server
./target/release/backend-service --db ./mydb --port 8080
```

**Endpoints:**

| Method | Path | Description |
|--------|------|-------------|
| GET | /api/health | Health check |
| POST | /api/sql | Execute SQL statement |
| POST | /api/tx/begin | Begin transaction |
| POST | /api/tx/{id}/commit | Commit transaction |
| POST | /api/tx/{id}/abort | Abort transaction |

**Example with curl:**

```bash
# Health check
curl http://localhost:8080/api/health

# Execute SQL
curl -X POST http://localhost:8080/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"CREATE TABLE users (id INT PRIMARY KEY, name TEXT)"}'

# Insert data
curl -X POST http://localhost:8080/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"INSERT INTO users VALUES (1, '\''Alice'\'')"}'

# Query data
curl -X POST http://localhost:8080/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql":"SELECT * FROM users"}'
```

**Transaction example:**

```bash
# Begin transaction
TX_ID=$(curl -s -X POST http://localhost:8080/api/tx/begin | jq -r '.result.tx_id')

# Execute operations within transaction
curl -X POST http://localhost:8080/api/sql \
  -H "Content-Type: application/json" \
  -d "{\"sql\":\"UPDATE users SET name='Bob' WHERE id=1\", \"tx_id\":\"$TX_ID\"}"

# Commit transaction
curl -X POST http://localhost:8080/api/tx/$TX_ID/commit

# Or abort
# curl -X POST http://localhost:8080/api/tx/$TX_ID/abort
```

### Using Docker

```bash
# Build the image
docker build -t rdbms:latest .

# Run the server
docker run -d \
  --name rdbms \
  -p 5432:5432 \
  -v /path/to/data:/data \
  rdbms:latest

# Connect to the server
docker exec -it rdbms rdbms --db /data
```

### Using Docker Compose

```yaml
version: '3.8'

services:
  rdbms:
    image: docker.io/reinhardb/rdbms:latest
    ports:
      - "5432:5432"
    volumes:
      - ./data:/data
    environment:
      - RDBMS_DATA_DIR=/data
    restart: unless-stopped
```

## Usage

### REPL Mode

```
$ ./target/release/rdbms --db ./mydb
RDBMS REPL v0.4.0
Using database file: ./mydb

rdbms> CREATE TABLE users (id INT PRIMARY KEY, name TEXT, email TEXT);
OK

rdbms> INSERT INTO users VALUES (1, 'Alice', 'alice@example.com');
INSERT 0 1

rdbms> SELECT * FROM users;
+----+-------+------------------+
| id | name  | email            |
+----+-------+------------------+
| 1  | Alice | alice@example.com|
+----+-------+------------------+
(1 row)

rdbms> EXIT
```

### TCP Server Mode

The TCP server accepts JSON-RPC style requests:

```json
// Request: Ping
{"method": "ping"}

// Response
{"status": "ok", "result": {"version": "0.4.0"}, "error": null}

// Request: Execute SQL (no result set)
{"method": "execute", "params": ["CREATE TABLE t (id INT)"]}

// Request: Query SQL (returns rows)
{"method": "execute", "params": ["SELECT * FROM t"]}
```

Example using netcat:

```bash
echo '{"method": "ping"}' | nc localhost 5432
echo '{"method": "execute", "params": ["SELECT 1 as value"]}' | nc localhost 5432
```

### Using Make Commands

```bash
# Build all binaries
make build-release

# Run tests
make test

# Run REPL
make run-repl

# Run TCP server
make run-server

# Run REST API server
make run-backend-service

# Build and run Docker image
make docker-build
make docker-run
```

### Programmatic Usage

```rust
use db::Engine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new("./mydb")?;

    // Execute DDL/DML
    engine.execute("CREATE TABLE users (id INT PRIMARY KEY, name TEXT)")?;

    // Query data
    let rows = engine.query("SELECT * FROM users")?;
    for row in rows {
        println!("{:?}", row);
    }

    Ok(())
}
```

## Configuration

### Command Line Options

#### REPL (`rdbms`)
```
Usage: rdbms [OPTIONS]

Options:
  --db <PATH>      Database directory [default: data]
  --help           Print help
```

#### Server (`rdbmsd`)
```
Usage: rdbmsd [OPTIONS]

Options:
  --db <PATH>        Database file [default: ./data.db]
  --listen <ADDR>    Listen address [default: 0.0.0.0:5432]
  --workers <N>      Number of worker threads (optional)
  --help             Print help
```

#### REST API (`backend-service`)
```
Usage: backend-service [OPTIONS]

Options:
  -d, --db <PATH>     Database file [default: ./data.db]
  -p, --port <PORT>   Port to listen on [default: 8080]
  --bind <ADDR>       Bind address [default: 0.0.0.0]
  --help              Print help
```

## Supported SQL

### Data Definition

```sql
CREATE TABLE table_name (
    column_name data_type [constraints],
    ...
);

DROP TABLE table_name;

ALTER TABLE table_name ADD COLUMN column_name data_type;
ALTER TABLE table_name RENAME TO new_name;
ALTER TABLE table_name DROP COLUMN column_name;
```

### Data Manipulation

```sql
INSERT INTO table_name VALUES (value1, value2, ...);
INSERT INTO table_name (col1, col2) VALUES (v1, v2);

UPDATE table_name SET col = value WHERE condition;

DELETE FROM table_name WHERE condition;

SELECT * FROM table_name [WHERE condition] [ORDER BY col] [LIMIT n];
```

### Indexes

```sql
CREATE INDEX index_name ON table_name (column_name);
CREATE UNIQUE INDEX index_name ON table_name (column_name);
CREATE PRIMARY KEY ON table_name (column_name);
```

### Transactions

```sql
BEGIN;
-- Your operations
COMMIT;

-- Or rollback
ROLLBACK;
```

## Project Structure

```
rdbms/
├── Cargo.toml                 # Workspace manifest
├── Cargo.lock                 # Dependency lockfile
├── Dockerfile                 # Container image definition
├── Makefile                   # Development commands
├── README.md                  # This file
├── crates/
│   ├── common/               # Shared utilities
│   ├── db/                   # Database engine (CLI, server)
│   ├── query/                # Query processor (SQL, execution)
│   ├── storage/              # Storage layer (buffer pool, disk)
│   ├── txn/                  # Transaction manager (locks, ACID)
│   └── wal/                  # Write-Ahead Log
├── services/
│   └── backend-service/      # REST API service (HTTP)
├── packaging/
│   └── systemd/              # Systemd service files
├── docs/                     # Documentation
├── .github/
│   └── workflows/
│       └── ci.yml            # CI/CD pipeline
└── tests/                    # Integration tests
```

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Rust](https://www.rust-lang.org/) - Systems programming language
- [tokio](https://tokio.rs/) - Async runtime
- [parking_lot](https://github.com/Amanieu/parking_lot) - Synchronization primitives
