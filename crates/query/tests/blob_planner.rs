use query::{sql_to_logical_plan, DataType, LogicalPlan};

#[test]
fn plan_blob_column_type() {
    let plan = sql_to_logical_plan("CREATE TABLE files (id INT, payload BLOB)").unwrap();
    match plan {
        LogicalPlan::CreateTable { columns, .. } => {
            assert_eq!(columns[1].data_type, DataType::Blob);
        }
        other => panic!("unexpected plan: {other:?}"),
    }
}

#[test]
fn blob_primary_key_is_rejected() {
    let err = sql_to_logical_plan("CREATE TABLE files (payload BLOB PRIMARY KEY)").unwrap_err();
    assert!(err
        .to_string()
        .contains("BLOB columns cannot be PRIMARY KEY or UNIQUE"));
}

#[test]
fn blob_unique_is_rejected() {
    let err = sql_to_logical_plan("CREATE TABLE files (payload BLOB UNIQUE)").unwrap_err();
    assert!(err
        .to_string()
        .contains("BLOB columns cannot be PRIMARY KEY or UNIQUE"));
}

#[test]
fn blob_default_is_rejected() {
    let err = sql_to_logical_plan("CREATE TABLE files (payload BLOB DEFAULT X'00')").unwrap_err();
    assert!(err.to_string().contains("BLOB columns cannot have DEFAULT"));
}
