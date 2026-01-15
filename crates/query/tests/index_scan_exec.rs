mod common;

use common::{make_catalog_with_users_table, run_sql, temp_buffer_pool};
use query::execution::ExecutionResult;
use query::{Tuple, Value};

fn user_tuple(id: i64, name: &str, email: &str) -> Tuple {
    Tuple::new(vec![
        Value::Integer(id),
        Value::String(name.to_string()),
        Value::String(email.to_string()),
    ])
}

#[test]
fn index_scan_returns_correct_tuple() -> ExecutionResult<()> {
    let buffer_pool = temp_buffer_pool();
    let (mut catalog, _) = make_catalog_with_users_table(buffer_pool);
    catalog
        .table_mut("users")
        .unwrap()
        .create_index("users_pk", "id", true, true)?;

    for id in 0..1000 {
        let name = format!("user-{}", id);
        let email = format!("user{}@example.com", id);
        catalog.insert_tuple("users", &user_tuple(id, &name, &email))?;
    }

    let results = run_sql(&catalog, "SELECT * FROM users WHERE id = 424");
    let expected = vec![user_tuple(424, "user-424", "user424@example.com")];
    assert_eq!(results, expected);

    let results = run_sql(&catalog, "SELECT * FROM users WHERE id = 9999");
    assert!(results.is_empty());
    Ok(())
}

#[test]
fn index_scan_projection_matches_baseline() -> ExecutionResult<()> {
    let buffer_pool = temp_buffer_pool();
    let (mut catalog, _) = make_catalog_with_users_table(buffer_pool);
    catalog
        .table_mut("users")
        .unwrap()
        .create_index("users_pk", "id", true, true)?;

    for id in 0..1000 {
        let name = format!("user-{}", id);
        let email = format!("user{}@example.com", id);
        catalog.insert_tuple("users", &user_tuple(id, &name, &email))?;
    }

    let results = run_sql(&catalog, "SELECT name FROM users WHERE id = 7");
    assert_eq!(
        results,
        vec![Tuple::new(vec![Value::String("user-7".to_string())])]
    );

    let table = catalog.table("users").unwrap();
    let baseline: Vec<Tuple> = table
        .heap
        .scan_tuples(&table.schema)?
        .into_iter()
        .map(|(_, tuple)| tuple)
        .filter(|tuple| matches!(tuple.get(0), Some(Value::Integer(7))))
        .map(|tuple| Tuple::new(vec![tuple.get(1).cloned().unwrap()]))
        .collect();
    assert_eq!(results, baseline);
    Ok(())
}
