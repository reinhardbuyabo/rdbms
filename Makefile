.PHONY: help build build-release test test-all clean run-repl run-server run-backend-service run-docker stop-docker docker-compose-build docker-compose-up docker-compose-up-with-init docker-compose-down docker-compose-logs docker-compose-logs-service docker-compose-restart docker-compose-clean docs install-systemd db-init db-init-api db-reset frontend-build-dev frontend-build-prod frontend-run-dev docker-compose-frontend-build docker-compose-frontend-up docker-compose-frontend-down

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
	@echo "Frontend Commands:"
	@echo "  make frontend-build-dev   - Build frontend development Docker image"
	@echo "  make frontend-build-prod  - Build frontend production Docker image"
	@echo "  make frontend-run-dev     - Run frontend dev server (port 5173)"
	@echo "  make frontend-stop        - Stop frontend containers"
	@echo "  make docker-compose-frontend-build - Build frontend with docker-compose"
	@echo "  make docker-compose-frontend-up   - Start frontend with docker-compose"
	@echo "  make docker-compose-frontend-down - Stop frontend docker-compose services"
	@echo ""
	@echo "Database Initialization:"
	@echo "  make db-init        - Initialize DB with schema and seed data"
	@echo "  make db-init DB_PATH=./data.db"
	@echo "                      - Initialize custom database path"
	@echo "  make db-init-api    - Initialize DB via REST API (requires running service)"
	@echo "  make db-reset       - Delete database file (for fresh start)"
	@echo ""
	@echo "Docker Commands:"
	@echo "  make docker-build           - Build Docker image"
	@echo "  make docker-run             - Run server in Docker"
	@echo "  make stop-docker            - Stop Docker containers"
	@echo ""
	@echo "Docker Compose Commands:"
	@echo "  make docker-compose-build       - Build all services with docker-compose"
	@echo "  make docker-compose-up          - Start all services (TCP + REST API)"
	@echo "  make docker-compose-up-with-init- Start services and initialize database"
	@echo "  make docker-compose-down        - Stop all services"
	@echo "  make docker-compose-logs        - Show logs for all services"
	@echo "  make docker-compose-restart     - Restart all services"
	@echo "  make docker-compose-clean       - Stop services and remove volumes"
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
	@echo "Loading environment from /home/reinhard/jan-capstone/.env"
	@(set -a && . /home/reinhard/jan-capstone/.env && set +a && sh -c 'echo "GOOGLE_CLIENT_ID: $$(echo "$$GOOGLE_CLIENT_ID" | cut -c1-20)..." && echo "JWT_SECRET: $$(echo "$$JWT_SECRET" | cut -c1-10)..."')
	@echo "Starting backend-service on port $(PORT)"
	@echo "Database: $(DB_PATH)"
	@(set -a && . /home/reinhard/jan-capstone/.env && set +a && DB_PATH=$(DB_PATH) PORT=$(PORT) cargo run -p backend_service)

run-backend-service-release: build-release
	@echo "$(GREEN)Starting backend-service on port $(PORT)$(NC)"
	@echo "$(GREEN)Database: $(DB_PATH)$(NC)"
	@DB_PATH=$(DB_PATH) PORT=$(PORT) ./target/release/backend_service

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

# Docker Compose commands
docker-compose-build:
	@echo "$(YELLOW)Building Docker images with docker-compose...$(NC)"
	@docker compose build

docker-compose-up:
	@echo "$(YELLOW)Starting all services with docker-compose...$(NC)"
	@docker compose up -d
	@echo "$(GREEN)Services started!$(NC)"
	@echo "  - TCP Server:  localhost:5432"
	@echo "  - REST API:    localhost:8080"
	@echo "  - Health:      http://localhost:8080/api/health"

docker-compose-up-with-init:
	@echo "$(YELLOW)Starting all services with database initialization...$(NC)"
	@docker compose up -d rdbmsd backend-service
	@echo "$(GREEN)Waiting for services to be healthy...$(NC)"
	@docker compose up -d db-init
	@echo "$(GREEN)Database initialized!$(NC)"
	@echo "  - TCP Server:  localhost:5432"
	@echo "  - REST API:    localhost:8080"

docker-compose-down:
	@echo "$(YELLOW)Stopping all Docker Compose services...$(NC)"
	@docker compose down
	@echo "$(GREEN)Services stopped$(NC)"

docker-compose-logs:
	@echo "$(YELLOW)Showing logs for all services...$(NC)"
	@docker compose logs -f

docker-compose-logs-service:
	@echo "$(YELLOW)Showing logs for $(SERVICE)...$(NC)"
	@docker compose logs -f $(SERVICE)

docker-compose-restart:
	@echo "$(YELLOW)Restarting all services...$(NC)"
	@docker compose restart
	@echo "$(GREEN)Services restarted$(NC)"

docker-compose-clean:
	@echo "$(YELLOW)Stopping services and removing volumes...$(NC)"
	@docker compose down -v
	@echo "$(GREEN)All data removed$(NC)"

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

# Database initialization
db-init: build
	@echo "$(GREEN)Initializing database with schema and seed data...$(NC)"
	@./scripts/db_init.sh DB_PATH=$(DB_PATH) REPL=./target/debug/rdbms

db-init-api:
	@echo "$(GREEN)Initializing database via API...$(NC)"
	@echo "$(YELLOW)Make sure backend-service is running on port $(PORT)$(NC)"
	@./scripts/seed_via_api.sh API_URL=http://localhost:$(PORT)

db-reset:
	@echo "$(YELLOW)Resetting database (removing existing data)...$(NC)"
	@rm -f $(DB_PATH) $(DB_PATH).wal $(DB_PATH).catalog 2>/dev/null || true
	@echo "$(GREEN)Database reset complete. Run 'make db-init' to reinitialize.$(NC)"

# Install binaries to /usr/local/bin
install:
	@echo "$(YELLOW)Installing RDBMS binaries to /usr/local/bin...$(NC)"
	@cp target/release/rdbms /usr/local/bin/rdbms 2>/dev/null || echo "Run 'make build-release' first"
	@cp target/release/rdbmsd /usr/local/bin/rdbmsd 2>/dev/null || echo "Run 'make build-release' first"
	@cp target/release/backend_service /usr/local/bin/backend-service 2>/dev/null || echo "Run 'make build-release' first"
	@echo "$(GREEN)Installed!$(NC)"
	@echo "Run with: rdbms --help, rdbmsd --help, or backend-service --help"

# Frontend build commands
frontend-build-dev:
	@echo "$(YELLOW)Building frontend development image...$(NC)"
	@docker build -t eventify:frontend-dev services/frontend --target development

frontend-build-prod:
	@echo "$(YELLOW)Building frontend production image...$(NC)"
	@docker build -t eventify:frontend-prod services/frontend --target production

frontend-run-dev:
	@echo "$(YELLOW)Starting frontend development server...$(NC)"
	@docker run -d \
		--name eventify-frontend-dev \
		-p 5173:5173 \
		-v $(PWD)/services/frontend:/app \
		-v /app/node_modules \
		-e VITE_API_BASE_URL=http://localhost:8080 \
		eventify:frontend-dev
	@echo "$(GREEN)Frontend dev server running at localhost:5173$(NC)"

frontend-stop:
	@echo "$(YELLOW)Stopping frontend containers...$(NC)"
	@docker stop eventify-frontend-dev 2>/dev/null || true
	@docker rm eventify-frontend-dev 2>/dev/null || true
	@echo "$(GREEN)Frontend containers stopped$(NC)"

# Docker Compose frontend commands
docker-compose-frontend-build:
	@echo "$(YELLOW)Building frontend with docker-compose...$(NC)"
	@docker compose build frontend-dev frontend

docker-compose-frontend-up:
	@echo "$(YELLOW)Starting frontend services with docker-compose...$(NC)"
	@docker compose up -d frontend-dev
	@echo "$(GREEN)Frontend dev server running at localhost:5173$(NC)"

docker-compose-frontend-down:
	@echo "$(YELLOW)Stopping frontend Docker Compose services...$(NC)"
	@docker compose stop frontend-dev frontend
	@docker compose rm -f frontend-dev frontend 2>/dev/null || true
	@echo "$(GREEN)Frontend services stopped$(NC)"
