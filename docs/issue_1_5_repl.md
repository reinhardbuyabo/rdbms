# Issue 1.5: Interactive SQL REPL

## Quick start

```bash
cargo run --bin rdbms -- --db data
```

- Prompt: `rdbms> ` (continuation uses `...> `)
- Exit: `\q`, `quit`, `exit`, `.exit`
- Help: `\help`
- Tables: `\tables`
- Schema: `\schema <table>`

## REPL behavior

- Multiline input continues until a statement terminator (`;`) is seen outside quotes/comments.
- Multiple statements in one paste are executed sequentially.
- SQL comments are supported:
  - Line comments: `-- comment until newline`
  - Block comments: `/* comment */` (unterminated blocks keep the REPL in continuation mode)
  - Comment markers are ignored inside quoted strings/identifiers.
- History is persisted between sessions:
  - `XDG_STATE_HOME/rdbms/history` or `~/.local/state/rdbms/history` if available
  - Fallback: `./.rdbms_history`

## Output formatting

- Results are printed in tables with headers and row counts.
- Large result sets are truncated at `MAX_DISPLAY_ROWS = 100` with a message:
  - `... (N rows hidden)`

## Implementation details

### Entry point

- Binary: `crates/db/src/bin/rdbms.rs`
- CLI arg: `--db <path>` (default `data`)

### REPL loop

- `crates/db/src/repl.rs`
  - Reads input via `rustyline` with history persistence.
  - Uses `split_statements` to detect complete SQL statements.
  - Dispatches meta commands before SQL execution.

### SQL splitter

- `crates/db/src/sql.rs`
- Finite-state machine with explicit states:
  - `Normal`, `SingleQuote`, `DoubleQuote`, `LineComment`, `BlockComment`
- Semicolons are only terminators in `Normal`.
- Unterminated block comments or quotes keep `in_string` true to continue input.

### Execution adapter

- `crates/db/src/engine.rs`
- Uses query crate APIs:
  - `sql_to_logical_plan` for parsing/planning
  - `PhysicalPlanner` + `Executor` for execution
- Handles `CREATE TABLE`, `INSERT`, `DELETE`, `UPDATE`, and query output formatting.

### Output printer

- `crates/db/src/printer.rs`
- Uses `comfy-table` for table formatting.
- Enforces the 100-row soft display limit.

## Tests

- Splitter unit tests: `crates/db/src/sql.rs`
- Printer unit tests: `crates/db/src/printer.rs`
- Engine safety regression test: `crates/db/src/engine.rs`

Run all tests:

```bash
cargo test
```
