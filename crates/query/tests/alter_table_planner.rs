use query::{sql_to_logical_plan, DataType, LogicalPlan};

#[test]
fn plan_alter_table_rename() {
    let plan = sql_to_logical_plan("ALTER TABLE people RENAME TO users").unwrap();
    assert!(matches!(
        plan,
        LogicalPlan::AlterTableRename {
            table_name,
            new_table_name
        } if table_name == "people" && new_table_name == "users"
    ));
}

#[test]
fn plan_alter_table_rename_column() {
    let plan = sql_to_logical_plan("ALTER TABLE people RENAME COLUMN name TO full_name").unwrap();
    assert!(matches!(
        plan,
        LogicalPlan::AlterTableRenameColumn {
            table_name,
            old_column_name,
            new_column_name
        } if table_name == "people" && old_column_name == "name" && new_column_name == "full_name"
    ));
}

#[test]
fn plan_alter_table_add_column() {
    let plan = sql_to_logical_plan("ALTER TABLE people ADD COLUMN age INT").unwrap();
    match plan {
        LogicalPlan::AlterTableAddColumn {
            table_name,
            column_def,
        } => {
            assert_eq!(table_name, "people");
            assert_eq!(column_def.name, "age");
            assert_eq!(column_def.data_type, DataType::Integer);
            assert!(column_def.nullable);
            assert!(!column_def.primary_key);
            assert!(!column_def.unique);
        }
        other => panic!("unexpected plan: {other:?}"),
    }
}

#[test]
fn plan_alter_table_drop_column() {
    let plan = sql_to_logical_plan("ALTER TABLE people DROP COLUMN age").unwrap();
    assert!(matches!(
        plan,
        LogicalPlan::AlterTableDropColumn {
            table_name,
            column_name
        } if table_name == "people" && column_name == "age"
    ));
}

#[test]
fn plan_alter_table_rejects_multiple_operations() {
    let err =
        sql_to_logical_plan("ALTER TABLE people ADD COLUMN age INT, DROP COLUMN name").unwrap_err();
    assert!(err
        .to_string()
        .contains("ALTER TABLE only supports a single operation"));
}

#[test]
fn plan_alter_table_rejects_unsupported_operation() {
    let err = sql_to_logical_plan("ALTER TABLE people DROP PRIMARY KEY").unwrap_err();
    assert!(err
        .to_string()
        .contains("Unsupported ALTER TABLE operation"));
}
