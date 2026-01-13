# Query Processing Layer (Phase 2)

This crate implements SQL parsing and logical query planning for the RDBMS project.

## Architecture

```
SQL Text → Parser → AST → Planner → LogicalPlan → [Phase 3: Executor]
```

## Components

### Parser (`parser.rs`)
- Thin wrapper around `sqlparser-rs`
- Handles SQL syntax validation
- Produces Abstract Syntax Tree (AST)

### Planner (`planner.rs`)
- Translates AST to LogicalPlan
- Performs semantic validation
- Resolves table aliases
- Validates column references

### Logical Plan (`logical_plan.rs`)
- Tree of relational algebra operators
- Represents query execution semantics
- Supports: Scan, Filter, Project, Join, Sort, Limit, Aggregate
- Supports: Insert, Update, Delete, CreateTable, DropTable

### Expression Trees (`expr.rs`)
- Represents SQL expressions
- Used in WHERE, SELECT, JOIN ON, etc.
- Supports: columns, literals, binary/unary ops, functions

### Schema (`schema.rs`)
- Type system and metadata
- Column definitions
- Data flow information

## Usage

```rust
use query::sql_to_logical_plan;

let sql = "SELECT * FROM users WHERE age > 18";
let plan = sql_to_logical_plan(sql)?;

println!("{}", plan.explain());
```

## Supported SQL

- `CREATE TABLE` with column types and constraints
- `INSERT INTO` with multiple rows
- `SELECT` with:
  - WHERE clause
  - JOIN (INNER, LEFT, RIGHT, FULL, CROSS)
  - ORDER BY
  - LIMIT/OFFSET
  - GROUP BY + aggregates (COUNT, SUM, AVG, MIN, MAX)
- `UPDATE` with WHERE clause
- `DELETE` with WHERE clause
- `DROP TABLE`

## Testing

```bash
cargo test --package query
cargo test --package query --test acceptance
```

## Next Phase

Phase 3 will implement the physical execution engine that consumes these logical plans.
