use db::engine::Engine;
use tempfile::TempDir;

fn create_test_engine() -> (Engine, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let engine = Engine::new(&db_path).unwrap();
    (engine, temp_dir)
}

#[test]
fn test_create_index_basic() {
    let (mut engine, _temp_dir) = create_test_engine();

    // Create a table first
    engine
        .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT, email TEXT)")
        .unwrap();

    // Create an index
    let result = engine.execute_sql("CREATE INDEX idx_users_name ON users(name)");
    assert!(result.is_ok(), "CREATE INDEX should succeed: {:?}", result);
    match result.unwrap() {
        db::printer::ReplOutput::Message(msg) => assert_eq!(msg, "OK"),
        _ => panic!("Expected Message output"),
    }
}

#[test]
fn test_create_index_if_not_exists() {
    let (mut engine, _temp_dir) = create_test_engine();

    engine
        .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT)")
        .unwrap();

    // Create index first time
    engine
        .execute_sql("CREATE INDEX idx_users_name ON users(name)")
        .unwrap();

    // Create index with IF NOT EXISTS - should succeed
    let result = engine.execute_sql("CREATE INDEX IF NOT EXISTS idx_users_name ON users(name)");
    assert!(result.is_ok(), "CREATE INDEX IF NOT EXISTS should succeed");
    match result.unwrap() {
        db::printer::ReplOutput::Message(msg) => assert_eq!(msg, "OK"),
        _ => panic!("Expected Message output"),
    }
}

#[test]
fn test_create_index_duplicate_error() {
    let (mut engine, _temp_dir) = create_test_engine();

    engine
        .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT)")
        .unwrap();

    // Create index first time
    engine
        .execute_sql("CREATE INDEX idx_users_name ON users(name)")
        .unwrap();

    // Create same index again - should fail
    let result = engine.execute_sql("CREATE INDEX idx_users_name ON users(name)");
    assert!(
        result.is_err(),
        "CREATE INDEX should fail for duplicate index"
    );
}

#[test]
fn test_create_index_on_nonexistent_table() {
    let (mut engine, _temp_dir) = create_test_engine();

    let result = engine.execute_sql("CREATE INDEX idx_nonexistent ON nonexistent(column)");
    assert!(
        result.is_err(),
        "CREATE INDEX should fail on nonexistent table"
    );
}

#[test]
fn test_create_index_on_nonexistent_column() {
    let (mut engine, _temp_dir) = create_test_engine();

    engine
        .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT)")
        .unwrap();

    let result =
        engine.execute_sql("CREATE INDEX idx_users_nonexistent ON users(nonexistent_column)");
    assert!(
        result.is_err(),
        "CREATE INDEX should fail on nonexistent column"
    );
}

#[test]
fn test_create_unique_index() {
    let (mut engine, _temp_dir) = create_test_engine();

    engine
        .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, email TEXT)")
        .unwrap();

    let result = engine.execute_sql("CREATE UNIQUE INDEX idx_users_email ON users(email)");
    assert!(result.is_ok(), "CREATE UNIQUE INDEX should succeed");
    match result.unwrap() {
        db::printer::ReplOutput::Message(msg) => assert_eq!(msg, "OK"),
        _ => panic!("Expected Message output"),
    }
}

#[test]
fn test_create_index_with_custom_name() {
    let (mut engine, _temp_dir) = create_test_engine();

    engine
        .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT)")
        .unwrap();

    let result = engine.execute_sql("CREATE INDEX my_custom_index ON users(name)");
    assert!(
        result.is_ok(),
        "CREATE INDEX with custom name should succeed"
    );
}

#[test]
fn test_create_multiple_indexes() {
    let (mut engine, _temp_dir) = create_test_engine();

    engine
        .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT, email TEXT, age INT)")
        .unwrap();

    // Create multiple indexes
    engine
        .execute_sql("CREATE INDEX idx_users_name ON users(name)")
        .unwrap();
    engine
        .execute_sql("CREATE INDEX idx_users_email ON users(email)")
        .unwrap();
    engine
        .execute_sql("CREATE INDEX idx_users_age ON users(age)")
        .unwrap();

    // Verify all indexes were created
    let result =
        engine.execute_sql("INSERT INTO users VALUES (1, 'Alice', 'alice@example.com', 30)");
    assert!(result.is_ok());

    let result = engine.execute_sql("INSERT INTO users VALUES (2, 'Bob', 'bob@example.com', 25)");
    assert!(result.is_ok());

    // Query using different columns - indexes should help
    let result = engine.execute_sql("SELECT * FROM users WHERE name = 'Alice'");
    assert!(result.is_ok(), "Query with index should succeed");

    let result = engine.execute_sql("SELECT * FROM users WHERE email = 'bob@example.com'");
    assert!(result.is_ok(), "Query with unique index should succeed");

    let result = engine.execute_sql("SELECT * FROM users WHERE age > 25");
    assert!(result.is_ok(), "Query with index should succeed");
}

#[test]
fn test_create_index_idempotency() {
    let (mut engine, _temp_dir) = create_test_engine();

    engine
        .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT)")
        .unwrap();

    // Create index multiple times with IF NOT EXISTS
    for _ in 0..3 {
        let result = engine.execute_sql("CREATE INDEX IF NOT EXISTS idx_users_name ON users(name)");
        assert!(
            result.is_ok(),
            "CREATE INDEX IF NOT EXISTS should be idempotent"
        );
    }
}

#[test]
fn test_create_index_then_insert() {
    let (mut engine, _temp_dir) = create_test_engine();

    engine
        .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT)")
        .unwrap();

    // Create index before inserting data
    engine
        .execute_sql("CREATE INDEX idx_users_name ON users(name)")
        .unwrap();

    // Insert data
    for i in 1..=100 {
        engine
            .execute_sql(&format!("INSERT INTO users VALUES ({}, 'User{}')", i, i))
            .unwrap();
    }

    // Query using the index
    let result = engine.execute_sql("SELECT * FROM users WHERE name = 'User50'");
    assert!(result.is_ok(), "Query using index should succeed");
}

#[test]
fn test_create_index_after_data() {
    let (mut engine, _temp_dir) = create_test_engine();

    engine
        .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT)")
        .unwrap();

    // Insert data first
    for i in 1..=100 {
        engine
            .execute_sql(&format!("INSERT INTO users VALUES ({}, 'User{}')", i, i))
            .unwrap();
    }

    // Create index after data exists
    let result = engine.execute_sql("CREATE INDEX idx_users_name ON users(name)");
    assert!(
        result.is_ok(),
        "CREATE INDEX on populated table should succeed"
    );

    // Query should still work
    let result = engine.execute_sql("SELECT * FROM users WHERE name = 'User50'");
    assert!(result.is_ok(), "Query should work after index creation");
}

#[test]
fn test_create_index_on_primary_key() {
    let (mut engine, _temp_dir) = create_test_engine();

    // Create table with primary key - index is auto-created
    engine
        .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT)")
        .unwrap();

    // Create additional index on same column
    let result = engine.execute_sql("CREATE INDEX idx_users_id ON users(id)");
    assert!(result.is_ok(), "CREATE INDEX on PK column should succeed");
}

#[test]
fn test_schema_sql_create_index() {
    let (mut engine, _temp_dir) = create_test_engine();

    // Test the actual schema.sql CREATE INDEX statements
    let schema_sql = r#"
        CREATE TABLE users (
            id TEXT PRIMARY KEY,
            role TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE events (
            id TEXT PRIMARY KEY,
            organizer_id TEXT NOT NULL,
            title TEXT NOT NULL,
            starts_at TEXT NOT NULL,
            published INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_events_organizer_id ON events(organizer_id);
    "#;

    for stmt in schema_sql.split(';').filter(|s| !s.trim().is_empty()) {
        let result = engine.execute_sql(stmt);
        assert!(result.is_ok(), "Schema statement should succeed: {}", stmt);
    }

    // Verify tables and index exist
    let result = engine.execute_sql("SELECT COUNT(*) FROM users");
    assert!(result.is_ok());

    let result = engine.execute_sql("SELECT COUNT(*) FROM events");
    assert!(result.is_ok());
}
