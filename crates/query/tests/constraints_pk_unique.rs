mod common;

use common::{make_catalog_with_users_table, temp_buffer_pool};
use query::execution::{ExecutionError, ExecutionResult};
use query::index::{Index, IndexKey};
use query::{sql_to_logical_plan, Executor, PhysicalPlanner, Tuple, Value};

fn user_tuple(id: i64, name: &str, email: &str) -> Tuple {
    Tuple::new(vec![
        Value::Integer(id),
        Value::String(name.to_string()),
        Value::String(email.to_string()),
    ])
}

#[test]
fn duplicate_primary_key_insert_fails_without_heap_leak() -> ExecutionResult<()> {
    let buffer_pool = temp_buffer_pool();
    let (mut catalog, _) = make_catalog_with_users_table(buffer_pool);
    catalog
        .table_mut("users")
        .unwrap()
        .create_index("users_pk", "id", true, true)?;

    let first = user_tuple(1, "Alice", "alice@example.com");
    catalog.insert_tuple("users", &first)?;

    let result = catalog.insert_tuple("users", &user_tuple(1, "Bob", "bob@example.com"));
    match result {
        Err(ExecutionError::ConstraintViolation {
            table,
            constraint,
            key,
        }) => {
            assert_eq!(table, "users");
            assert_eq!(constraint, "users_pk");
            assert_eq!(key, "1");
        }
        other => {
            return Err(ExecutionError::Execution(format!(
                "expected constraint violation, got {:?}",
                other
            )));
        }
    }

    let table = catalog.table("users").unwrap();
    let tuples = table.heap.scan_tuples(&table.schema)?;
    assert_eq!(tuples.len(), 1);

    let index = table
        .indexes
        .iter()
        .find(|index| index.name == "users_pk")
        .unwrap();
    let rids = index.index.get(&IndexKey::Integer(1))?;
    assert_eq!(rids.len(), 1);
    Ok(())
}

#[test]
fn duplicate_unique_insert_fails_without_heap_leak() -> ExecutionResult<()> {
    let buffer_pool = temp_buffer_pool();
    let (mut catalog, _) = make_catalog_with_users_table(buffer_pool);
    catalog
        .table_mut("users")
        .unwrap()
        .create_index("users_email_unique", "email", true, false)?;

    let first = user_tuple(1, "Alice", "dup@example.com");
    let second = user_tuple(2, "Bob", "dup@example.com");
    catalog.insert_tuple("users", &first)?;

    let result = catalog.insert_tuple("users", &second);
    assert!(matches!(
        result,
        Err(ExecutionError::ConstraintViolation { .. })
    ));

    let table = catalog.table("users").unwrap();
    let tuples = table.heap.scan_tuples(&table.schema)?;
    assert_eq!(tuples.len(), 1);

    let index = table
        .indexes
        .iter()
        .find(|index| index.name == "users_email_unique")
        .unwrap();
    let rids = index
        .index
        .get(&IndexKey::Text("dup@example.com".to_string()))?;
    assert_eq!(rids.len(), 1);
    Ok(())
}

#[test]
fn update_moves_index_entry() -> ExecutionResult<()> {
    let buffer_pool = temp_buffer_pool();
    let (mut catalog, _) = make_catalog_with_users_table(buffer_pool);
    catalog
        .table_mut("users")
        .unwrap()
        .create_index("users_pk", "id", true, true)?;

    catalog.insert_tuple("users", &user_tuple(5, "Alice", "alice@example.com"))?;

    let logical = sql_to_logical_plan("UPDATE users SET id = 500 WHERE id = 5").unwrap();
    let root = PhysicalPlanner::new(&catalog).plan(&logical)?;
    let mut executor = Executor::new(root);
    let results = executor.execute()?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], user_tuple(500, "Alice", "alice@example.com"));

    let table = catalog.table("users").unwrap();
    let index = table
        .indexes
        .iter()
        .find(|index| index.name == "users_pk")
        .unwrap();
    assert!(index.index.get(&IndexKey::Integer(5))?.is_empty());
    let rids = index.index.get(&IndexKey::Integer(500))?;
    assert_eq!(rids.len(), 1);
    let tuple = table.heap.get_tuple(rids[0], &table.schema)?;
    assert_eq!(tuple, Some(user_tuple(500, "Alice", "alice@example.com")));
    Ok(())
}

#[test]
fn update_rejects_duplicate_key() -> ExecutionResult<()> {
    let buffer_pool = temp_buffer_pool();
    let (mut catalog, _) = make_catalog_with_users_table(buffer_pool);
    catalog
        .table_mut("users")
        .unwrap()
        .create_index("users_pk", "id", true, true)?;

    catalog.insert_tuple("users", &user_tuple(5, "Alice", "alice@example.com"))?;
    catalog.insert_tuple("users", &user_tuple(7, "Bob", "bob@example.com"))?;

    let logical = sql_to_logical_plan("UPDATE users SET id = 5 WHERE id = 7").unwrap();
    let root = PhysicalPlanner::new(&catalog).plan(&logical)?;
    let mut executor = Executor::new(root);
    let result = executor.execute();
    assert!(matches!(
        result,
        Err(ExecutionError::ConstraintViolation { .. })
    ));

    let table = catalog.table("users").unwrap();
    let tuples = table.heap.scan_tuples(&table.schema)?;
    let ids: Vec<i64> = tuples
        .into_iter()
        .filter_map(|(_, tuple)| match tuple.get(0) {
            Some(Value::Integer(value)) => Some(*value),
            _ => None,
        })
        .collect();
    assert!(ids.contains(&5));
    assert!(ids.contains(&7));
    Ok(())
}
