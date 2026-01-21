# Eventify - Event Ticketing Platform

A full-stack event ticketing platform built with Rust (backend) and React/TypeScript (frontend). Features include Google OAuth authentication, event management, ticket sales, and shopping cart functionality.

![CI](https://github.com/reinhardbuyabo/rdbms/workflows/CI/badge.svg)
![License](https://img.shields.io/github/license/reinhardbuyabo/rdbms)
![Version](https://img.shields.io/github/v/release/reinhardbuyabo/rdbms)

## Features

- **Backend (Rust)**:
  - ACID Transactions with full transaction support
  - Write-Ahead Logging (WAL) for crash recovery
  - Lock Manager with deadlock detection
  - B+Tree Indexes for efficient queries
  - REST API for frontend integration

- **Frontend (React/TypeScript)**:
  - Modern SPA with Vite
  - Google OAuth authentication
  - Event browsing and management
  - Ticket purchase with credit card validation
  - Shopping cart with localStorage persistence

## Architecture

```
┌───────────────────────────────────────────────────────────────────────────────┐
│                           Eventify Platform                                   │
├───────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│   ┌───────────────────────────────────────────────────────────────────────┐   │
│   │                        Frontend (React/Vite)                          │   │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────────┐    │   │
│   │  │  User App   │  │  Organizer  │  │        Admin Panel          │    │   │
│   │  │  (Browse)   │  │   (Manage)  │  │        (Dashboard)          │    │   │
│   │  └─────────────┘  └─────────────┘  └─────────────────────────────┘    │   │
│   │         │                │                     │                      │   │
│   │         └────────────────┼─────────────────────┘                      │   │
│   │                          ▼                                            │   │
│   │  ┌─────────────────────────────────────────────────────────────────┐  │   │
│   │  │                      REST API (HTTP)                            │  │   │
│   │  │          GET /events, POST /orders, GET /auth/*                 │  │   │
│   │  └───────────────────────────┬─────────────────────────────────────┘  │   │
│   └──────────────────────────────┼────────────────────────────────────────┘   │
│                                  │                                            │
│   ┌──────────────────────────────┼────────────────────────────────────────┐   │
│   │                              ▼                                        │   │
│   │  ┌──────────────────────────────────────────────────────────────────┐ │   │
│   │  │                    Backend Service (Rust)                        │ │   │
│   │  ├──────────────────────────────────────────────────────────────────┤ │   │
│   │  │  ┌───────────┐  ┌───────────┐  ┌──────────────────────────────┐  │ │   │
│   │  │  │   Auth    │  │   Event   │  │         Order/Ticket         │  │ │   │
│   │  │  │  Handler  │  │  Manager  │  │           Manager            │  │ │   │
│   │  │  └─────┬─────┘  └─────┬─────┘  └───────────────┬──────────────┘  │ │   │
│   │  │        │              │                        │                 │ │   │
│   │  │        └──────────────┼────────────────────────┘                 │ │   │
│   │  │                       ▼                                          │ │   │
│   │  │  ┌────────────────────────────────────────────────────────────┐  │ │   │
│   │  │  │                     RDBMS Engine                           │  │ │   │
│   │  │  ├────────────────────────────────────────────────────────────┤  │ │   │
│   │  │  │  ┌───────────┐  ┌───────────┐  ┌──────────────────────────┐│  │ │   │
│   │  │  │  │  Catalog  │  │   Lock    │  │    TransactionManager    ││  │ │   │
│   │  │  │  │           │  │  Manager  │  │                          ││  │ │   │
│   │  │  │  └───────────┘  └───────────┘  └──────────────────────────┘│  │ │   │
│   │  │  │  ┌───────────┐  ┌───────────┐  ┌──────────────────────────┐│  │ │   │
│   │  │  │  │   Query   │  │  Recovery │  │     BufferPoolManager    ││  │ │   │
│   │  │  │  │   Engine  │  │  Manager  │  │                          ││  │ │   │
│   │  │  │  └───────────┘  └───────────┘  └──────────────────────────┘│  │ │   │
│   │  │  └────────────────────────────────────────────────────────────┘  │ │   │
│   │  │                     │              │              │              │ │   │
│   │  │                     ▼              ▼              ▼              │ │   │
│   │  │  ┌──────────────────────────────────────────────────────────────┐│ │   │
│   │  │  │              Storage Layer (Disk + Buffer Pool)              ││ │   │
│   │  │  └──────────────────────────────────────────────────────────────┘│ │   │
│   │  └──────────────────────────────────────────────────────────────────┘ │   │
│   │                              │                                        │   │
│   │                              ▼                                        │   │
│   │  ┌──────────────────────────────────────────────────────────────────┐ │   │
│   │  │              Write-Ahead Log (WAL) - Crash Recovery              │ │   │
│   │  └──────────────────────────────────────────────────────────────────┘ │   │
│   └───────────────────────────────────────────────────────────────────────┘   │
│                                                                               │
└───────────────────────────────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

- Rust 1.70 or later
- Cargo
- Docker & Docker Compose
- Node.js 20+ (for frontend development)

### Option 1: Docker Compose (Recommended)

The fastest way to get the full stack running:

```bash
# Clone the repository
git clone https://github.com/reinhardbuyabo/rdbms.git
cd rdbms

# Build and start all services
docker compose up -d

# Or with database initialization
docker compose up -d rdbmsd backend-service
docker compose up -d db-init

# View logs
docker compose logs -f

# Stop all services
docker compose down

# Stop and remove volumes (data loss!)
docker compose down -v
```

**Services started:**
- `frontend-dev` - Frontend dev server on port 5173 (with hot reload)
- `frontend` - Frontend production on port 80
- `rdbmsd` - TCP server on port 5432
- `backend-service` - REST API on port 8080
- `db-init` - Database initialization (runs once)

### Option 2: Local Development

**Backend (Rust):**

```bash
# Build all binaries
cargo build --release

# Run the RDBMS server (TCP)
./target/release/rdbmsd --db ./data.db --listen 0.0.0.0:5432

# Run the REST API server
./target/release/backend-service --db ./data.db --port 8080
```

**Frontend (React/Vite):**

```bash
cd services/frontend

# Install dependencies
npm install

# Start development server with hot reload
npm run dev

# Build for production
npm run build
```

## Frontend Development

### Commands

```bash
cd services/frontend

# Development mode with hot reload
npm run dev

# Development mode accessible externally
npm run dev -- --host 0.0.0.0

# Type checking
npm run typecheck

# Linting
npm run lint

# Build for production
npm run build

# Preview production build locally
npm run preview
```

### Docker Frontend Commands

```bash
# Build development image
make frontend-build-dev

# Build production image
make frontend-build-prod

# Run dev server
make frontend-run-dev

# Stop frontend containers
make frontend-stop

# Using docker-compose
make docker-compose-frontend-build
make docker-compose-frontend-up
make docker-compose-frontend-down
```

### Environment Variables

Create a `.env` file in `services/frontend/`:

```bash
VITE_API_BASE_URL=http://localhost:8080
VITE_GOOGLE_CLIENT_ID=your-google-client-id
```

## Backend API Reference

### REST API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | /api/health | Health check |
| POST | /api/sql | Execute SQL statement |
| POST | /api/tx/begin | Begin transaction |
| POST | /api/tx/{id}/commit | Commit transaction |
| POST | /api/tx/{id}/abort | Abort transaction |
| GET | /auth/google/start | Start Google OAuth flow |
| GET | /auth/google/callback | OAuth callback handler |
| GET | /me | Get current user profile (requires JWT) |

### OAuth Authentication

The backend-service supports Google OAuth 2.0 authentication.

**Configuration:**

```bash
export GOOGLE_CLIENT_ID="your-google-client-id.apps.googleusercontent.com"
export GOOGLE_CLIENT_SECRET="your-google-client-secret"
export GOOGLE_REDIRECT_URI="http://localhost:8080/auth/google/callback"
export JWT_SECRET="your-super-secret-key"
export JWT_TTL_SECONDS="3600"
```

**OAuth Flow:**

1. Visit `/auth/google/start` to initiate OAuth flow
2. User authenticates with Google
3. Google redirects to `/auth/google/callback?code=AUTHORIZATION_CODE`
4. Server exchanges code for tokens, creates/updates user, returns JWT

## Project Structure

```
/home/reinhard/jan-capstone/
├── Cargo.toml                 # Rust workspace manifest
├── Cargo.lock                 # Dependency lockfile
├── Dockerfile                 # Backend container image
├── Makefile                   # Development commands
├── README.md                  # This file
├── docker-compose.yml         # Full stack orchestration
│
├── crates/                    # Rust crates
│   ├── common/               # Shared utilities
│   ├── db/                   # Database engine (CLI, server)
│   ├── query/                # Query processor (SQL, execution)
│   ├── storage/              # Storage layer (buffer pool, disk)
│   ├── txn/                  # Transaction manager (locks, ACID)
│   └── wal/                  # Write-Ahead Log
│
├── services/
│   ├── backend-service/      # REST API service (Rust/Actix-web)
│   └── frontend/             # React/Vite frontend (TypeScript)
│       ├── src/
│       │   ├── components/   # React components
│       │   ├── pages/        # Page components
│       │   ├── context/      # React context (Auth, Cart)
│       │   ├── lib/          # Utilities
│       │   └── api/          # API client
│       ├── Dockerfile        # Frontend container image
│       ├── nginx.conf        # Production nginx config
│       └── package.json      # Node dependencies
│
├── packaging/
│   └── systemd/              # Systemd service files
├── docs/                     # Documentation
└── tests/                    # Integration tests
```

## Supported SQL

See [docs/SQL.md](docs/SQL.md) for full SQL syntax reference.

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
- [React](https://react.dev/) - UI framework
- [Vite](https://vitejs.dev/) - Build tool
- [Actix-web](https://actix.rs/) - Rust web framework
