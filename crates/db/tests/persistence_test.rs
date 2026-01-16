use db::engine::Engine;
use tempfile::TempDir;

#[test]
fn test_concurrent_visibility_same_engine() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut engine = Engine::new(&db_path).unwrap();

    engine
        .execute_sql("CREATE TABLE test (id INT PRIMARY KEY, data TEXT)")
        .unwrap();
    engine
        .execute_sql("INSERT INTO test VALUES (1, 'hello')")
        .unwrap();

    let result = engine.execute_sql("SELECT * FROM test").unwrap();
    assert!(
        result.to_string().contains("hello"),
        "Should see inserted data"
    );
}

#[test]
fn test_table_creation_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    {
        let mut engine1 = Engine::new(&db_path).unwrap();
        engine1
            .execute_sql("CREATE TABLE users (id INT, name TEXT)")
            .unwrap();
        engine1
            .execute_sql("INSERT INTO users VALUES (1, 'alice')")
            .unwrap();
    }

    let mut engine2 = Engine::new(&db_path).unwrap();
    let result = engine2.execute_sql("SELECT * FROM users").unwrap();
    assert!(
        result.to_string().contains("alice"),
        "Should see data from first engine"
    );
}

#[test]
fn test_persistence_across_engine_instances() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    {
        let mut engine1 = Engine::new(&db_path).unwrap();
        engine1
            .execute_sql("CREATE TABLE test (id INT PRIMARY KEY, data TEXT)")
            .unwrap();
        engine1
            .execute_sql("INSERT INTO test VALUES (1, 'from_api')")
            .unwrap();
    }

    let mut engine2 = Engine::new(&db_path).unwrap();
    let result = engine2.execute_sql("SELECT * FROM test").unwrap();
    assert!(
        result.to_string().contains("from_api"),
        "Should see data written by first engine"
    );
}

#[test]
fn test_multiple_inserts_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut engine = Engine::new(&db_path).unwrap();
    engine
        .execute_sql("CREATE TABLE users (id INT, name TEXT)")
        .unwrap();

    for i in 1..=3 {
        engine
            .execute_sql(&format!("INSERT INTO users VALUES ({}, 'user{}')", i, i))
            .unwrap();
    }

    let result = engine.execute_sql("SELECT * FROM users").unwrap();
    assert!(
        result.to_string().contains("user1"),
        "Should have inserted rows"
    );

    drop(engine);
    let mut engine2 = Engine::new(&db_path).unwrap();
    let result = engine2.execute_sql("SELECT * FROM users").unwrap();
    assert!(
        result.to_string().contains("user1"),
        "Rows should persist after restart"
    );
    assert!(
        result.to_string().contains("user2"),
        "All rows should persist"
    );
    assert!(
        result.to_string().contains("user3"),
        "All rows should persist"
    );
}

#[test]
fn test_update_and_delete_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut engine = Engine::new(&db_path).unwrap();
    engine
        .execute_sql("CREATE TABLE test (id INT PRIMARY KEY, value INT)")
        .unwrap();
    engine
        .execute_sql("INSERT INTO test VALUES (1, 100)")
        .unwrap();
    engine
        .execute_sql("INSERT INTO test VALUES (2, 200)")
        .unwrap();

    engine
        .execute_sql("UPDATE test SET value = 999 WHERE id = 1")
        .unwrap();
    let result = engine
        .execute_sql("SELECT value FROM test WHERE id = 1")
        .unwrap();
    assert!(result.to_string().contains("999"), "Update should persist");

    engine.execute_sql("DELETE FROM test WHERE id = 2").unwrap();
    let result = engine.execute_sql("SELECT * FROM test").unwrap();
    assert!(
        !result.to_string().contains("200"),
        "Deleted row should not appear"
    );

    drop(engine);
    let mut engine2 = Engine::new(&db_path).unwrap();
    let result = engine2.execute_sql("SELECT * FROM test").unwrap();
    assert!(
        result.to_string().contains("999"),
        "Update should persist after restart"
    );
    assert!(
        !result.to_string().contains("200"),
        "Delete should persist after restart"
    );
}

#[test]
fn test_schema_persistence_with_primary_key() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    {
        let mut engine = Engine::new(&db_path).unwrap();
        engine
            .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, email TEXT UNIQUE, name TEXT)")
            .unwrap();
        engine
            .execute_sql("INSERT INTO users VALUES (1, 'alice@example.com', 'Alice')")
            .unwrap();
        engine
            .execute_sql("INSERT INTO users VALUES (2, 'bob@example.com', 'Bob')")
            .unwrap();
    }

    let mut engine2 = Engine::new(&db_path).unwrap();
    let result = engine2.execute_sql("SELECT * FROM users").unwrap();
    assert!(
        result.to_string().contains("alice@example.com"),
        "Data should persist with primary key"
    );
    assert!(
        result.to_string().contains("bob@example.com"),
        "All rows should persist"
    );

    let err = engine2.execute_sql("INSERT INTO users VALUES (3, 'alice@example.com', 'Eve')");
    assert!(
        err.is_err(),
        "Unique constraint should be enforced after restart"
    );

    let err = engine2.execute_sql("INSERT INTO users VALUES (1, 'charlie@example.com', 'Charlie')");
    assert!(
        err.is_err(),
        "Primary key constraint should be enforced after restart"
    );
}

#[test]
fn test_schema_persistence_with_default_values() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    {
        let mut engine = Engine::new(&db_path).unwrap();
        engine
            .execute_sql(
                "CREATE TABLE products (id INT PRIMARY KEY, name TEXT, price INT DEFAULT 0)",
            )
            .unwrap();
        engine
            .execute_sql("INSERT INTO products (id, name) VALUES (1, 'Widget')")
            .unwrap();
        engine
            .execute_sql("INSERT INTO products (id, name, price) VALUES (2, 'Gadget', 100)")
            .unwrap();
    }

    let mut engine2 = Engine::new(&db_path).unwrap();
    let result = engine2.execute_sql("SELECT * FROM products").unwrap();
    assert!(
        result.to_string().contains("Widget"),
        "Product should persist"
    );
    assert!(
        result.to_string().contains("Gadget"),
        "All products should persist"
    );
}

#[test]
fn test_concurrent_client_visibility() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    {
        let mut engine1 = Engine::new(&db_path).unwrap();
        engine1
            .execute_sql("CREATE TABLE items (id INT PRIMARY KEY, name TEXT)")
            .unwrap();
        engine1
            .execute_sql("INSERT INTO items VALUES (1, 'first')")
            .unwrap();
    }

    {
        let mut engine2 = Engine::new(&db_path).unwrap();
        let result = engine2.execute_sql("SELECT * FROM items").unwrap();
        assert!(
            result.to_string().contains("first"),
            "Engine2 should see data from engine1"
        );

        engine2
            .execute_sql("INSERT INTO items VALUES (2, 'second')")
            .unwrap();
    }

    let mut engine3 = Engine::new(&db_path).unwrap();
    let result = engine3.execute_sql("SELECT * FROM items").unwrap();
    assert!(
        result.to_string().contains("first") && result.to_string().contains("second"),
        "All data should persist across multiple engine instances"
    );
}
