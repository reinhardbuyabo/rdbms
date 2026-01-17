# AGENTS.md - Development Guidelines for RDBMS Project

## Build Commands

```bash
# Debug build
make build

# Release build
make build-release

# Run all tests
make test

# Run specific integration tests
cargo test -p db --test persistence_test
cargo test -p txn --test acid_full_tests
cargo test -p txn --test transaction_core_tests

# Run single test
cargo test test_name          # in workspace root
cargo test -p db test_name    # in specific crate

# Run REPL
make run-repl DB_PATH=./data.db

# Run server
make run-server PORT=5432 DB_PATH=./data.db

# Run backend-service (REST API)
make run-backend-service PORT=8080

# Clean build artifacts
make clean
```

## Linting & Formatting

```bash
# Check formatting
cargo fmt --all -- --check

# Format code
cargo fmt --all

# Run clippy (strict warnings)
cargo clippy --workspace -- -D warnings

# Full CI check
cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings && cargo test --workspace
```

## Project Structure

```
/home/reinhard/jan-capstone/
├── crates/          # Core Rust crates
│   ├── common/      # Shared utilities
│   ├── db/          # Main DB engine, REPL, server binaries
│   ├── query/       # SQL parser, planner, query execution
│   ├── storage/     # Buffer pool, disk manager, B+Tree index
│   ├── txn/         # Transaction manager, lock manager, WAL
│   └── wal/         # Write-Ahead Log
├── services/
│   └── backend-service/  # REST API server (Actix-web)
└── Makefile         # Build automation
```

## Code Style Guidelines

### Imports
- Use absolute paths with `crate::` for internal imports
- Group imports: std → external crates → `crate::` modules
- Prefer named imports over glob imports (`use std::collections::HashMap;` not `use std::collections::*;`)
- Use `#[allow(unused_imports)]` sparingly at crate root if needed

### Formatting
- Follow `rustfmt` defaults (4 spaces, 100 char line width)
- Use vertical alignment for struct fields with same-types
- Empty line between `use` statements and code
- One blank line between function definitions in a `impl` block

### Types & Naming
- **Newtype wrappers**: Use tuple struct wrapper for type safety (e.g., `TxnId(u64)`, `PageId(u64)`)
- **Enums**: Use `enum` for state/discriminated types with `#[derive(Debug, Clone, PartialEq)]`
- **Result types**: Use `Result<T, SomeError>` not alias unless generic
- **Error types**: Define specific error enums per crate (e.g., `LockError`, `BufferPoolError`)
- **Type aliases**: Use for common Result patterns (`pub type BufferPoolResult<T> = Result<T, BufferPoolError>`)
- **Snake_case** for functions, variables, modules
- **PascalCase** for types, traits, enum variants
- **SCREAMING_SNAKE_CASE** for constants

### Error Handling
- Use `anyhow::Result<T>` at application boundaries (main, handlers, CLI)
- Define custom error enums for library/internal errors
- Use `?` operator for propagation
- Use `.context()` from anyhow for additional context on errors
- Never `unwrap()` or `expect()` except in tests

### Concurrency & Synchronization
- Use `parking_lot::Mutex` (not std `Mutex`) for performance
- Use `Arc<Mutex<T>>` for shared mutable state across threads
- Use `Arc<Barrier>` for synchronization in tests
- Document thread-safety requirements on public types

### Testing
- Unit tests go in `#[cfg(test)] mod tests` within the same file
- Integration tests go in `tests/` directory per crate
- Use `tempfile` for test databases
- Tests should be fast and isolated (create own test DB)
- Use `assert!`, `assert_eq!`, `assert_matches!` for assertions

### Documentation
- Document all public types and functions with `///` doc comments
- Document `Error` enum variants
- Include examples in doc comments where helpful
- Use `#[warn(missing_docs)]` lint (enforced by CI)

### SQL & Database Patterns
- Use `sql_to_logical_plan()` for parsing SQL to query plan
- Use `Catalog` for schema metadata management
- Follow `Executor` trait pattern for query execution operators
- Always use `PageGuard` for buffer pool access (RAII pattern)

### REST API (backend-service)
- Use `actix-web` framework
- Return `AnyhowResult<Json<T>>` from handlers
- Use `web::Data<AppState>` for shared engine state
- Always wrap with `context()` for error context
- Enable CORS for development: `Cors::default().allow_any_origin()`

### Git Conventions
- Branch naming: `feat/*`, `fix/*`, `docs/*`
- Commit messages: present tense, imperative mood ("Add lock timeout")
- Run `cargo fmt` and `cargo clippy` before committing

- Never read, log, or display contents of .env or any file containing secrets.
