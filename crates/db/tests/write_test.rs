use db::engine::Engine;
use tempfile::TempDir;

#[test]
fn test_insert_after_fix() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut engine = Engine::new(&db_path).unwrap();

    // Create table
    let result = engine.execute_sql("CREATE TABLE test (id INT PRIMARY KEY, data TEXT)");
    assert!(result.is_ok(), "CREATE TABLE should succeed: {:?}", result);

    // Insert data
    let result = engine.execute_sql("INSERT INTO test VALUES (1, 'hello')");
    assert!(result.is_ok(), "INSERT should succeed: {:?}", result);

    // Verify data
    let result = engine.execute_sql("SELECT * FROM test");
    assert!(result.is_ok(), "SELECT should succeed: {:?}", result);
}
