use db::engine::Engine;
use tempfile::TempDir;

#[test]
fn test_engine_via_api_pattern() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut engine = Engine::new(&db_path).unwrap();

    engine
        .execute_sql("CREATE TABLE api_test (id INT, name TEXT)")
        .unwrap();
    engine
        .execute_sql("INSERT INTO api_test VALUES (1, 'test1')")
        .unwrap();
    engine
        .execute_sql("INSERT INTO api_test VALUES (2, 'test2')")
        .unwrap();

    let result = engine.execute_sql("SELECT * FROM api_test").unwrap();
    let output = result.to_string();
    assert!(output.contains("test1"), "Should contain test1");
    assert!(output.contains("test2"), "Should contain test2");
}

#[test]
fn test_api_sql_execution_pattern() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut engine = Engine::new(&db_path).unwrap();

    let test_cases = vec![
        ("CREATE TABLE t (id INT PRIMARY KEY, name TEXT)", true),
        ("INSERT INTO t VALUES (1, 'alice')", true),
        ("INSERT INTO t VALUES (2, 'bob')", true),
        ("SELECT * FROM t", true),
        ("UPDATE t SET name = 'charlie' WHERE id = 1", true),
        ("DELETE FROM t WHERE id = 2", true),
        ("INSERT INTO t VALUES (1, 'duplicate')", false),
    ];

    for (sql, should_succeed) in test_cases {
        let result = engine.execute_sql(sql);
        if should_succeed {
            assert!(result.is_ok(), "SQL should succeed: {}", sql);
        } else {
            assert!(result.is_err(), "SQL should fail: {}", sql);
        }
    }
}

#[test]
fn test_api_transaction_pattern() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut engine = Engine::new(&db_path).unwrap();

    engine
        .execute_sql("CREATE TABLE accounts (id INT PRIMARY KEY, balance INT)")
        .unwrap();

    engine
        .execute_sql("INSERT INTO accounts VALUES (1, 1000)")
        .unwrap();
    engine
        .execute_sql("INSERT INTO accounts VALUES (2, 500)")
        .unwrap();

    let result = engine.execute_sql("SELECT * FROM accounts").unwrap();
    let output = result.to_string();
    assert!(output.contains("1000"));
    assert!(output.contains("500"));

    engine
        .execute_sql("UPDATE accounts SET balance = 900 WHERE id = 1")
        .unwrap();
    engine
        .execute_sql("UPDATE accounts SET balance = 600 WHERE id = 2")
        .unwrap();

    let result = engine.execute_sql("SELECT * FROM accounts").unwrap();
    let output = result.to_string();
    assert!(output.contains("900"));
    assert!(output.contains("600"));
}

#[test]
fn test_api_schema_constraints() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut engine = Engine::new(&db_path).unwrap();

    let result = engine.execute_sql(
        "CREATE TABLE products (id INT PRIMARY KEY, sku TEXT UNIQUE, name TEXT, price INT DEFAULT 10)"
    );
    assert!(result.is_ok());

    let result =
        engine.execute_sql("INSERT INTO products (id, sku, name) VALUES (1, 'ABC', 'Product A')");
    assert!(result.is_ok());

    let result = engine.execute_sql("SELECT name, price FROM products");
    assert!(result.is_ok());

    let result =
        engine.execute_sql("INSERT INTO products (id, sku, name) VALUES (2, 'ABC', 'Duplicate')");
    assert!(result.is_err());
}

#[test]
fn test_api_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut engine = Engine::new(&db_path).unwrap();

    let error_cases = vec![
        "SELECT * FROM nonexistent_table",
        "INSERT INTO missing VALUES (1)",
        "INVALID SQL SYNTAX",
        "DROP TABLE nonexistent",
    ];

    for sql in error_cases {
        let result = engine.execute_sql(sql);
        assert!(result.is_err(), "Should error on: {}", sql);
    }
}

#[test]
fn test_multiple_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut engine = Engine::new(&db_path).unwrap();

    engine
        .execute_sql("CREATE TABLE test_vals (id INT PRIMARY KEY)")
        .unwrap();

    for i in 1..=50 {
        let sql = format!("INSERT INTO test_vals VALUES ({i})");
        engine.execute_sql(&sql).unwrap();
    }

    let result = engine.execute_sql("SELECT * FROM test_vals").unwrap();
    let output = result.to_string();
    assert!(output.contains("50"));
}

#[test]
fn test_persistence_via_engine() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    {
        let mut engine = Engine::new(&db_path).unwrap();
        engine
            .execute_sql("CREATE TABLE persisted (id INT, data TEXT)")
            .unwrap();
        engine
            .execute_sql("INSERT INTO persisted VALUES (1, 'first')")
            .unwrap();
        engine
            .execute_sql("INSERT INTO persisted VALUES (2, 'second')")
            .unwrap();
    }

    {
        let mut engine = Engine::new(&db_path).unwrap();
        let result = engine.execute_sql("SELECT * FROM persisted").unwrap();
        let output = result.to_string();
        assert!(output.contains("first"));
        assert!(output.contains("second"));
    }
}
