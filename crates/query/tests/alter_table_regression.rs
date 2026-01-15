use query::sql_to_logical_plan;

#[test]
fn supported_statements_do_not_hit_unsupported() {
    let statements = vec![
        "CREATE TABLE users (id INT, name TEXT)",
        "DROP TABLE users",
        "INSERT INTO users (id, name) VALUES (1, 'Ada')",
        "SELECT * FROM users",
        "SELECT name FROM users WHERE id = 1",
        "UPDATE users SET name = 'Bob' WHERE id = 1",
        "DELETE FROM users WHERE id = 1",
        "ALTER TABLE users RENAME TO customers",
        "ALTER TABLE users RENAME COLUMN name TO full_name",
        "ALTER TABLE users ADD COLUMN age INT",
        "ALTER TABLE users DROP COLUMN name",
    ];

    for sql in statements {
        let result = sql_to_logical_plan(sql);
        assert!(result.is_ok(), "expected supported SQL: {sql}");
    }
}

#[test]
fn unsupported_statements_report_specific_errors() {
    let err = sql_to_logical_plan("ALTER INDEX idx RENAME TO idx2").unwrap_err();
    assert!(
        err.to_string().contains("Unsupported statement type"),
        "unexpected error: {err}"
    );

    let err = sql_to_logical_plan("ALTER TABLE users DROP PRIMARY KEY").unwrap_err();
    assert!(
        err.to_string()
            .contains("Unsupported ALTER TABLE operation"),
        "unexpected error: {err}"
    );
}
