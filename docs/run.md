# Running RDBMS

This document provides comprehensive instructions for running the RDBMS server in various modes.

## Table of Contents

1. [Building from Source](#building-from-source)
2. [Running the TCP Server](#running-the-tcp-server)
3. [Using the REPL](#using-the-repl)
4. [Running with Docker](#running-with-docker)
5. [API Reference](#api-reference)
6. [Testing](#testing)

---

## Building from Source

### Prerequisites

- Rust 1.70 or later
- Cargo
- Git

### Build Steps

```bash
# Clone the repository
git clone https://github.com/anomalyco/rdbms.git
cd rdbms

# Build all binaries (REPL and TCP server)
cargo build --release

# Verify binaries were created
ls -la target/release/rdbms
ls -la target/release/rdbmsd
```

---

## Running the TCP Server

The TCP server (`rdbmsd`) allows you to connect to RDBMS from any programming language via TCP sockets.

### Basic Usage

```bash
# Create database directory
mkdir -p /tmp/rdbms_data

# Start server on default port 5432
./target/release/rdbmsd --db /tmp/rdbms_data/mydb --listen 127.0.0.1:5432

# Start server on custom port
./target/release/rdbmsd --db /tmp/rdbms_data/mydb --listen 0.0.0.0:5432
```

### Running in Background

```bash
# Using nohup
nohup ./target/release/rdbmsd --db /tmp/rdbms_data/mydb --listen 0.0.0.0:5432 > /tmp/rdbms.log 2>&1 &

# Using systemd (create /etc/systemd/system/rdbms.service)
[Unit]
Description=RDBMS Database Server
After=network.target

[Service]
Type=simple
User=postgres
WorkingDirectory=/opt/rdbms
ExecStart=/opt/rdbms/target/release/rdbmsd --db /var/lib/rdbms --listen 0.0.0.0:5432
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

### Verifying the Server is Running

```bash
# Check if process is running
pgrep -af rdbmsd

# Check port is listening
lsof -i :5432

# Test connection
echo '{"method":"ping"}' | nc -w 2 127.0.0.1 5432
# Expected: {"status":"ok","result":{"version":"0.1.0"},"error":null}
```

---

## Using the REPL

The REPL provides an interactive terminal interface to the database.

### Starting the REPL

```bash
./target/release/rdbms --db /tmp/rdbms_data/mydb
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

-- List tables
.tables (not yet implemented)

-- Exit REPL
.exit
```

---

## Running with Docker

### Building the Docker Image

```bash
# Build the image
docker build -t rdbms:latest .

# Build with specific tag
docker build -t rdbms:v0.1.0 .
```

### Running the Container

```bash
# Run in detached mode
docker run -d \
  --name rdbms \
  -p 5432:5432 \
  -v $(pwd)/data:/data \
  rdbms:latest

# Run with custom port
docker run -d \
  --name rdbms \
  -p 5433:5432 \
  -v /home/user/rdbms_data:/data \
  rdbms:latest \
  --listen 0.0.0.0:5432

# Run interactively (for REPL)
docker run -it --rm \
  -v $(pwd)/data:/data \
  rdbms:latest \
  ./target/release/rdbms --db /data/mydb
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
      - "5432:5432"
    volumes:
      - ./data:/data
    environment:
      - RDBMS_DATA_DIR=/data
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "nc", "-z", "localhost", "5432"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 5s
```

Start the service:

```bash
docker-compose up -d
docker-compose logs -f rdbms
docker-compose down
```

### Connecting to Dockerized RDBMS

**Python:**

```python
import socket
import json

def execute(sql, host='localhost', port=5432):
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(5)
    sock.connect((host, port))
    request = {"method": "execute", "params": [sql]}
    sock.sendall(json.dumps(request).encode())
    sock.shutdown(socket.SHUT_WR)
    response = sock.recv(16384)
    sock.close()
    return json.loads(response.decode())

# Example usage
result = execute("SELECT * FROM users")
print(json.dumps(result, indent=2))
```

**Go:**

```go
package main

import (
    "encoding/json"
    "net"
    "fmt"
)

func execute(sql string) (map[string]interface{}, error) {
    conn, err := net.Dial("tcp", "localhost:5432")
    if err != nil {
        return nil, err
    }
    defer conn.Close()

    request := map[string]interface{}{
        "method": "execute",
        "params": []string{sql},
    }
    json.NewEncoder(conn).Encode(request)

    var response map[string]interface{}
    json.NewDecoder(conn).Decode(&response)
    return response, nil
}

func main() {
    result, _ := execute("SELECT * FROM users")
    jsonBytes, _ := json.MarshalIndent(result, "", "  ")
    fmt.Println(string(jsonBytes))
}
```

---

## API Reference

### Request Format

```json
{
  "method": "execute",
  "params": ["SQL_STATEMENT"]
}
```

### Response Format

**Success:**

```json
{
  "status": "ok",
  "result": {
    "message": "OK"
  },
  "error": null
}
```

**Query Result:**

```json
{
  "status": "ok",
  "result": {
    "columns": ["id", "name", "email"],
    "rows": [
      [{"type": "int", "value": 1}, {"type": "text", "value": "Alice"}, {"type": "text", "value": "alice@example.com"}]
    ]
  },
  "error": null
}
```

**Error:**

```json
{
  "status": "error",
  "result": null,
  "error": "table 'users' already exists"
}
```

### Supported Methods

| Method | Description | Example |
|--------|-------------|---------|
| `ping` | Health check, returns version | `{"method":"ping"}` |
| `execute` | Execute any SQL statement | `{"method":"execute","params":["SELECT * FROM users"]}` |

### Value Types

Query results use tagged JSON values:

| Type | Format | Example |
|------|--------|---------|
| Null | `{"type":"null"}` | `{"type":"null"}` |
| Integer | `{"type":"int","value":123}` | `{"type":"int","value":42}` |
| Float | `{"type":"float","value":3.14}` | `{"type":"float","value":1.5}` |
| Boolean | `{"type":"bool","value":true}` | `{"type":"bool","value":false}` |
| Text | `{"type":"text","value":"hello"}` | `{"type":"text","value":"world"}` |
| Blob | `{"type":"blob","value":"BASE64"}` | `{"type":"blob","value":"SGVsbG8="}` |

---

## Testing

### Integration Test Script

Create `test_rdbms.py`:

```python
#!/usr/bin/env python3
"""RDBMS Integration Test Script"""

import socket
import json
import sys

class RDBMSClient:
    def __init__(self, host='127.0.0.1', port=5432):
        self.host = host
        self.port = port

    def execute(self, sql):
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(5)
        sock.connect((self.host, self.port))
        request = {"method": "execute", "params": [sql]}
        sock.sendall(json.dumps(request).encode())
        sock.shutdown(socket.SHUT_WR)
        response = sock.recv(16384)
        sock.close()
        return json.loads(response.decode())

def test():
    client = RDBMSClient()

    # Test ping
    result = client.execute("SELECT 1 as test")
    assert result['status'] == 'ok', f"Ping failed: {result}"
    print("✓ Ping test passed")

    # Test DDL
    client.execute("DROP TABLE IF EXISTS test_users")
    client.execute("CREATE TABLE test_users (id INT PRIMARY KEY, name TEXT)")
    print("✓ CREATE TABLE test passed")

    # Test DML
    client.execute("INSERT INTO test_users VALUES (1, 'Alice')")
    client.execute("INSERT INTO test_users VALUES (2, 'Bob')")
    print("✓ INSERT test passed")

    # Test SELECT
    result = client.execute("SELECT * FROM test_users")
    assert result['status'] == 'ok', f"SELECT failed: {result}"
    assert len(result['result']['rows']) == 2, "Expected 2 rows"
    print("✓ SELECT test passed")

    # Test UPDATE
    client.execute("UPDATE test_users SET name = 'Alice Smith' WHERE id = 1")
    result = client.execute("SELECT name FROM test_users WHERE id = 1")
    print("✓ UPDATE test passed")

    # Test DELETE
    client.execute("DELETE FROM test_users WHERE id = 2")
    result = client.execute("SELECT COUNT(*) as cnt FROM test_users")
    print("✓ DELETE test passed")

    # Cleanup
    client.execute("DROP TABLE test_users")
    print("✓ DROP TABLE test passed")

    print("\n✓ All tests passed!")

if __name__ == "__main__":
    test()
```

Run the test:

```bash
python3 test_rdbms.py
```

### Automated Test Script

A comprehensive test script is included in the repository:

```bash
# Run the test script
./test_rdbms.sh
```

---

## Troubleshooting

### Server Won't Start

```bash
# Check if port is already in use
lsof -i :5432

# Kill existing process
pkill -f rdbmsd

# Check logs
cat /tmp/rdbms.log
```

### Connection Refused

```bash
# Verify server is running
pgrep -af rdbmsd

# Check firewall
ufw status

# Try localhost connection
nc -zv 127.0.0.1 5432
```

### Build Errors

```bash
# Clean build artifacts
cargo clean

# Rebuild
cargo build --release
```

---

## File Locations

| Component | Location |
|-----------|----------|
| REPL binary | `./target/release/rdbms` |
| TCP server binary | `./target/release/rdbmsd` |
| Database files | `/path/to/your/database/` |
| WAL logs | `/path/to/your/database.wal` |
| Docker data volume | `/data` (inside container) |
