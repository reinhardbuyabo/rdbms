mod common;

use common::temp_buffer_pool;
use query::{
    sql_to_logical_plan, Catalog, DataType, Field, PhysicalPlanner, Schema, TableHeap, TableInfo,
};

fn blob_schema(table: &str) -> Schema {
    Schema::new(vec![
        Field {
            name: "id".to_string(),
            table: Some(table.to_string()),
            data_type: DataType::Integer,
            nullable: false,
            visible: true,
        },
        Field {
            name: "payload".to_string(),
            table: Some(table.to_string()),
            data_type: DataType::Blob,
            nullable: true,
            visible: true,
        },
    ])
}

#[test]
fn blob_predicates_are_rejected() {
    let buffer_pool = temp_buffer_pool();
    let schema = blob_schema("files");
    let heap = TableHeap::create(buffer_pool).expect("create heap");
    let table = TableInfo::new("files", schema, heap);
    let mut catalog = Catalog::new();
    catalog.register_table_info(table);

    let logical = sql_to_logical_plan("SELECT * FROM files WHERE payload = X'FF'").unwrap();
    let result = PhysicalPlanner::new(&catalog).plan(&logical);
    assert!(result.is_err());
    let message = result.err().unwrap().to_string();
    assert!(message.contains("BLOB columns do not support predicate"));
}

#[test]
fn blob_join_predicates_are_rejected() {
    let buffer_pool = temp_buffer_pool();
    let schema_left = blob_schema("left_table");
    let schema_right = blob_schema("right_table");
    let heap_left = TableHeap::create(buffer_pool.clone()).expect("create left heap");
    let heap_right = TableHeap::create(buffer_pool).expect("create right heap");
    let left = TableInfo::new("left_table", schema_left, heap_left);
    let right = TableInfo::new("right_table", schema_right, heap_right);
    let mut catalog = Catalog::new();
    catalog.register_table_info(left);
    catalog.register_table_info(right);

    let logical = sql_to_logical_plan(
        "SELECT * FROM left_table JOIN right_table ON left_table.payload = right_table.payload",
    )
    .unwrap();
    let result = PhysicalPlanner::new(&catalog).plan(&logical);
    assert!(result.is_err());
    let message = result.err().unwrap().to_string();
    assert!(message.contains("BLOB columns do not support predicate"));
}
