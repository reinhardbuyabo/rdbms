#![allow(dead_code)]

use query::schema::{DataType, Field};
use query::{
    sql_to_logical_plan, Catalog, Executor, PhysicalPlanner, Rid, Schema, TableHeap, TableInfo,
    Tuple, Value,
};
use storage::{BufferPoolManager, DiskManager};
use tempfile::TempDir;

pub fn temp_buffer_pool() -> BufferPoolManager {
    let dir = TempDir::new().expect("temp dir create failed");
    let path = dir.path().join("db");
    let disk_manager = DiskManager::open(path.to_str().expect("temp path utf8")).unwrap();
    BufferPoolManager::new(disk_manager, 32)
}

pub fn users_schema() -> Schema {
    Schema::new(vec![
        Field {
            name: "id".to_string(),
            table: Some("users".to_string()),
            data_type: DataType::Integer,
            nullable: false,
        },
        Field {
            name: "name".to_string(),
            table: Some("users".to_string()),
            data_type: DataType::Text,
            nullable: false,
        },
        Field {
            name: "email".to_string(),
            table: Some("users".to_string()),
            data_type: DataType::Text,
            nullable: false,
        },
    ])
}

pub fn make_catalog_with_users_table(buffer_pool: BufferPoolManager) -> (Catalog, TableInfo) {
    let schema = users_schema();
    let heap = TableHeap::create(buffer_pool).expect("create table heap");
    let table = TableInfo::new("users", schema, heap);
    let mut catalog = Catalog::new();
    catalog.register_table_info(table.clone());
    (catalog, table)
}

pub fn insert_user(heap: &TableHeap, schema: &Schema, id: i64, name: &str, email: &str) -> Rid {
    let tuple = Tuple::new(vec![
        Value::Integer(id),
        Value::String(name.to_string()),
        Value::String(email.to_string()),
    ]);
    heap.insert_tuple(&tuple, schema).expect("insert user")
}

pub fn run_sql(catalog: &Catalog, sql: &str) -> Vec<Tuple> {
    let logical = sql_to_logical_plan(sql).expect("logical plan");
    let root = PhysicalPlanner::new(catalog)
        .plan(&logical)
        .expect("physical plan");
    let mut executor = Executor::new(root);
    executor.execute().expect("execute")
}
