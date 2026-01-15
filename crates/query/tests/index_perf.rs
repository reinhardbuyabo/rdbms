mod common;

use common::{make_catalog_with_users_table, temp_buffer_pool};
use query::execution::{ExecutionResult, Filter, IndexPredicate, IndexScan, SeqScan};
use query::index::IndexKey;
use query::{BinaryOperator, Executor, Expr, LiteralValue, Tuple, Value};

fn user_tuple(id: i64, name: &str, email: &str) -> Tuple {
    Tuple::new(vec![
        Value::Integer(id),
        Value::String(name.to_string()),
        Value::String(email.to_string()),
    ])
}

fn build_table(row_count: i64) -> (storage::BufferPoolManager, query::TableInfo) {
    let buffer_pool = temp_buffer_pool();
    let (mut catalog, _) = make_catalog_with_users_table(buffer_pool.clone());
    let table = catalog.table("users").unwrap();
    let schema = table.schema.clone();
    let heap = table.heap.clone();
    let name = "user";
    let email = "user@example.com";

    for id in 0..row_count {
        let _ = heap
            .insert_tuple(&user_tuple(id, name, email), &schema)
            .unwrap();
    }

    catalog
        .table_mut("users")
        .unwrap()
        .create_index("users_pk", "id", true, true)
        .unwrap();

    let table = catalog.table("users").unwrap().clone();
    (buffer_pool, table)
}

#[test]
fn index_scan_uses_fewer_page_fetches_than_seq_scan() -> ExecutionResult<()> {
    let row_count = 50_000;
    let target = 42_000;

    let (index_pool, index_table) = build_table(row_count);
    let expected = vec![user_tuple(target, "user", "user@example.com")];
    let index = index_table
        .indexes
        .iter()
        .find(|index| index.name == "users_pk")
        .unwrap()
        .index
        .clone();

    index_pool.reset_fetch_count();
    let predicate = IndexPredicate::equality(IndexKey::Integer(target));
    let mut executor = Executor::new(Box::new(IndexScan::new(
        index_table.heap.clone(),
        index_table.schema.clone(),
        index,
        predicate,
    )));
    let index_results = executor.execute()?;
    let index_fetches = index_pool.fetch_count();
    assert_eq!(index_results, expected);
    assert!(index_fetches > 0);

    let predicate = Expr::BinaryOp {
        left: Box::new(Expr::Column {
            table: Some("users".to_string()),
            name: "id".to_string(),
        }),
        op: BinaryOperator::Eq,
        right: Box::new(Expr::Literal(LiteralValue::Integer(target))),
    };
    let mut executor = Executor::new(Box::new(Filter::new(
        Box::new(SeqScan::new(
            index_table.heap.clone(),
            index_table.schema.clone(),
        )),
        predicate,
        index_table.schema.clone(),
    )));
    index_pool.reset_fetch_count();
    let seq_results = executor.execute()?;
    let seq_fetches = index_pool.fetch_count();
    assert_eq!(seq_results, expected);
    assert!(seq_fetches >= index_fetches.saturating_mul(20));
    Ok(())
}
