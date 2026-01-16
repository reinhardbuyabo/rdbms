mod common;

use common::temp_buffer_pool;
use query::execution::ExecutionError;
use query::{Catalog, ColumnDef, DataType, Field, Schema, TableHeap, TableInfo};

fn people_schema(table_name: &str) -> Schema {
    Schema::new(vec![
        Field {
            name: "id".to_string(),
            table: Some(table_name.to_string()),
            data_type: DataType::Integer,
            nullable: false,
            visible: true,
        },
        Field {
            name: "name".to_string(),
            table: Some(table_name.to_string()),
            data_type: DataType::Text,
            nullable: true,
            visible: true,
        },
        Field {
            name: "email".to_string(),
            table: Some(table_name.to_string()),
            data_type: DataType::Text,
            nullable: true,
            visible: true,
        },
    ])
}

fn catalog_with_people() -> (Catalog, TableInfo) {
    let buffer_pool = temp_buffer_pool();
    let schema = people_schema("people");
    let heap = TableHeap::create(buffer_pool).expect("create heap");
    let table = TableInfo::new("people", schema, heap);
    let mut catalog = Catalog::new();
    catalog.register_table_info(table.clone());
    (catalog, table)
}

#[test]
fn catalog_rename_table_updates_namespace() {
    let (mut catalog, _) = catalog_with_people();
    catalog.rename_table("people", "users").unwrap();

    assert!(catalog.table("people").is_none());
    let table = catalog.table("users").expect("users table");
    assert_eq!(table.name, "users");
    for field in &table.schema.fields {
        assert_eq!(field.table.as_deref(), Some("users"));
    }
}

#[test]
fn catalog_rename_column_preserves_order() {
    let (mut catalog, _) = catalog_with_people();
    catalog
        .rename_column("people", "name", "full_name")
        .unwrap();

    let table = catalog.table("people").expect("people table");
    let names = table
        .schema
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["id", "full_name", "email"]);
    assert!(table.schema.field_index("name").is_none());
    assert!(table.schema.field_index("full_name").is_some());
}

#[test]
fn catalog_add_column_appends() {
    let (mut catalog, _) = catalog_with_people();
    catalog
        .add_column(
            "people",
            ColumnDef {
                name: "age".to_string(),
                data_type: DataType::Integer,
                nullable: true,
                primary_key: false,
                unique: false,
                default_value: None,
            },
        )
        .unwrap();

    let table = catalog.table("people").expect("people table");
    let last = table.schema.fields.last().expect("last field");
    assert_eq!(last.name, "age");
    assert!(last.visible);
}

#[test]
fn catalog_drop_column_hides_field() {
    let (mut catalog, _) = catalog_with_people();
    catalog.drop_column("people", "email").unwrap();

    let table = catalog.table("people").expect("people table");
    let field = table
        .schema
        .fields
        .iter()
        .find(|field| field.name == "email")
        .expect("email field");
    assert!(!field.visible);
    assert!(table.schema.field_index("email").is_none());
    assert_eq!(table.schema.visible_schema().fields.len(), 2);
}

#[test]
fn catalog_drop_primary_key_is_rejected() {
    let buffer_pool = temp_buffer_pool();
    let schema = people_schema("accounts");
    let heap = TableHeap::create(buffer_pool).expect("create heap");
    let mut table = TableInfo::new("accounts", schema, heap);
    table
        .create_index("accounts_id_pk", "id", true, true)
        .expect("create index");
    let mut catalog = Catalog::new();
    catalog.register_table_info(table);

    let result = catalog.drop_column("accounts", "id");
    assert!(matches!(result, Err(ExecutionError::Schema(_))));
}
