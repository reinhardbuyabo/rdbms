.PHONY: help build build-release test test-all clean run-repl run-server run-backend-service run-docker stop-docker docs install-systemd

# Default settings
DB_PATH ?= ./data.db
PORT ?= 8080
SERVER_PORT ?= 5432
DOCKER_PORT ?= 8080

# Colors for output
GREEN = \033[0;32m
YELLOW = \033[1;33m
NC = \033[0m # No Color

help:
	@echo ""
	@echo "RDBMS Makefile - Quick commands for development"
	@echo ""
	@echo "Build Commands:"
	@echo "  make build          - Build debug binaries"
	@echo "  make build-release  - Build optimized release binaries"
	@echo "  make test           - Run all tests"
	@echo ""
	@echo "Run Commands:"
	@echo "  make run-repl       - Start REPL with default database"
	@echo "  make run-repl DB_PATH=/custom/path.db"
	@echo "                      - Start REPL with custom database"
	@echo "  make run-server     - Start RDBMS server (TCP listener)"
	@echo "  make run-server PORT=5432 DB_PATH=/var/lib/rdbms/db.db"
	@echo "                      - Start server with custom settings"
	@echo "  make run-backend-service  - Start backend-service (REST API for frontend)"
	@echo ""
	@echo "Docker Commands:"
	@echo "  make docker-build   - Build Docker image"
	@echo "  make docker-run     - Run server in Docker"
	@echo "  make stop-docker    - Stop Docker containers"
	@echo ""
	@echo "Systemd Commands:"
	@echo "  make install-systemd  - Install as systemd service (requires root)"
	@echo ""
	@echo "Examples:"
	@echo "  make run-repl DB_PATH=/tmp/test.db"
	@echo "  make run-server PORT=5432"
	@echo "  make docker-build && make docker-run"
	@echo ""

# Build commands
build:
	@cargo build

build-release:
	@cargo build --release
	@echo ""
	@echo "$(GREEN)Build complete!$(NC)"
	@ls -la target/release/rdbms 2>/dev/null && echo "REPL:  target/release/rdbms"
	@ls -la target/release/rdbmsd 2>/dev/null && echo "Server: target/release/rdbmsd"

# Test commands
test:
	@cargo test

test-all: test
	@echo ""
	@echo "$(YELLOW)Running integration tests...$(NC)"
	@cargo test -p db --test persistence_test
	@cargo test -p txn --test acid_full_tests
	@cargo test -p txn --test transaction_core_tests

# Run REPL
run-repl: build
	@echo "$(GREEN)Starting REPL with database: $(DB_PATH)$(NC)"
	@./target/debug/rdbms --db $(DB_PATH)

run-repl-release: build-release
	@echo "$(GREEN)Starting REPL with database: $(DB_PATH)$(NC)"
	@./target/release/rdbms --db $(DB_PATH)

# Run RDBMS Server (systemd managed binary)
run-server: build
	@echo "$(GREEN)Starting RDBMS server on port $(SERVER_PORT)$(NC)"
	@echo "$(GREEN)Database: $(DB_PATH)$(NC)"
	@DB_PATH=$(DB_PATH) ./target/debug/rdbmsd --db $(DB_PATH) --listen 0.0.0.0:$(SERVER_PORT)

run-server-release: build-release
	@echo "$(GREEN)Starting RDBMS server on port $(SERVER_PORT)$(NC)"
	@echo "$(GREEN)Database: $(DB_PATH)$(NC)"
	@DB_PATH=$(DB_PATH) ./target/release/rdbmsd --db $(DB_PATH) --listen 0.0.0.0:$(SERVER_PORT)

# Run Backend Service (REST API for frontend)
run-backend-service: build
	@echo "$(GREEN)Starting backend-service on port $(PORT)$(NC)"
	@echo "$(GREEN)Database: $(DB_PATH)$(NC)"
	@DB_PATH=$(DB_PATH) PORT=$(PORT) cargo run -p backend-service

run-backend-service-release: build-release
	@echo "$(GREEN)Starting backend-service on port $(PORT)$(NC)"
	@echo "$(GREEN)Database: $(DB_PATH)$(NC)"
	@DB_PATH=$(DB_PATH) PORT=$(PORT) ./target/release/backend-service

# Docker commands
docker-build:
	@echo "$(YELLOW)Building Docker image...$(NC)"
	@docker build -t rdbms:latest .

docker-run:
	@echo "$(YELLOW)Starting RDBMS server in Docker on port $(DOCKER_PORT)$(NC)"
	@docker run -d \
		--name rdbms-server \
		-p $(DOCKER_PORT):5432 \
		-v $(PWD)/data:/data \
		rdbms:latest
	@echo "$(GREEN)Server running at localhost:$(DOCKER_PORT)$(NC)"

docker-stop:
	@echo "$(YELLOW)Stopping Docker containers...$(NC)"
	@docker stop rdbms-server 2>/dev/null || true
	@docker rm rdbms-server 2>/dev/null || true
	@echo "$(GREEN)Docker containers stopped$(NC)"

# Systemd installation
install-systemd:
	@echo "$(YELLOW)Installing RDBMS as systemd service...$(NC)"
	@if [ "$(id -u)" -ne 0 ]; then echo "Error: This requires root. Run with sudo."; exit 1; fi
	@./packaging/systemd/install.sh

# Documentation
docs:
	@echo "$(YELLOW)Building documentation...$(NC)"
	@ls -la docs/
	@echo ""
	@echo "Quick start: cat docs/QUICKSTART.md"
	@echo "Full guide:  cat docs/run.md"

# Quick test scripts
test-crud:
	@echo "$(YELLOW)Testing CRUD operations via backend-service...$(NC)"
	@echo "1. Create table..."
	@curl -s -X POST http://localhost:$(PORT)/api/sql \
		-H "Content-Type: application/json" \
		-d '{"sql":"CREATE TABLE test_make (id INT PRIMARY KEY, name TEXT)"}'
	@echo ""
	@echo "2. Insert data..."
	@curl -s -X POST http://localhost:$(PORT)/api/sql \
		-H "Content-Type: application/json" \
		-d '{"sql":"INSERT INTO test_make VALUES (1, '\''Test'\'')"}'
	@echo ""
	@echo "3. Query data..."
	@curl -s -X POST http://localhost:$(PORT)/api/sql \
		-H "Content-Type: application/json" \
		-d '{"sql":"SELECT * FROM test_make"}'
	@echo ""

# Cleanup
clean:
	@echo "$(YELLOW)Cleaning build artifacts...$(NC)"
	@cargo clean
	@rm -rf data/*.db data/*.wal data/*.catalog 2>/dev/null || true
	@echo "$(GREEN)Clean complete$(NC)"

# Quick demo
demo: build
	@echo "$(GREEN)Running quick demo...$(NC)"
	@echo "This will create a demo database and run sample queries"
	@rm -rf /tmp/rdbms-demo.db 2>/dev/null || true
	@echo ""
	@echo "Creating table and inserting data..."
	@echo "CREATE TABLE demo (id INT, value TEXT);" | ./target/debug/rdbms --db /tmp/rdbms-demo.db 2>/dev/null || true
	@echo "INSERT INTO demo VALUES (1, 'Hello');" | ./target/debug/rdbms --db /tmp/rdbms-demo.db 2>/dev/null || true
	@echo "INSERT INTO demo VALUES (2, 'World');" | ./target/debug/rdbms --db /tmp/rdbms-demo.db 2>/dev/null || true
	@echo "SELECT * FROM demo;" | ./target/debug/rdbms --db /tmp/rdbms-demo.db 2>/dev/null || true
	@echo ""
	@echo "Demo database created at /tmp/rdbms-demo.db"

# Install binaries to /usr/local/bin
install:
	@echo "$(YELLOW)Installing RDBMS binaries to /usr/local/bin...$(NC)"
	@cp target/release/rdbms /usr/local/bin/rdbms 2>/dev/null || echo "Run 'make build-release' first"
	@cp target/release/rdbmsd /usr/local/bin/rdbmsd 2>/dev/null || echo "Run 'make build-release' first"
	@cp target/release/backend-service /usr/local/bin/backend-service 2>/dev/null || echo "Run 'make build-release' first"
	@echo "$(GREEN)Installed!$(NC)"
	@echo "Run with: rdbms --help, rdbmsd --help, or backend-service --help"
