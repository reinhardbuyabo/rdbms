mod common;

use common::{make_catalog_with_users_table, temp_buffer_pool};
use query::execution::{Filter, IndexScan, PhysicalOperator, Projection, SeqScan};
use query::{sql_to_logical_plan, PhysicalPlanner};

fn unwrap_projection<'a>(root: &'a Box<dyn PhysicalOperator>) -> &'a dyn PhysicalOperator {
    if let Some(projection) = root.as_any().downcast_ref::<Projection>() {
        projection.child()
    } else {
        &**root
    }
}

#[test]
fn indexscan_selected_for_equality_predicate() {
    let buffer_pool = temp_buffer_pool();
    let (mut catalog, _) = make_catalog_with_users_table(buffer_pool);
    catalog
        .table_mut("users")
        .unwrap()
        .create_index("users_pk", "id", true, true)
        .unwrap();

    let logical = sql_to_logical_plan("SELECT * FROM users WHERE id = 42").unwrap();
    let root = PhysicalPlanner::new(&catalog).plan(&logical).unwrap();
    let operator = unwrap_projection(&root);
    assert!(operator.as_any().is::<IndexScan>());

    let logical = sql_to_logical_plan("SELECT * FROM users u WHERE u.id = 42").unwrap();
    let root = PhysicalPlanner::new(&catalog).plan(&logical).unwrap();
    let operator = unwrap_projection(&root);
    assert!(operator.as_any().is::<IndexScan>());
}

#[test]
fn no_index_falls_back_to_filter_seqscan() {
    let buffer_pool = temp_buffer_pool();
    let (catalog, _) = make_catalog_with_users_table(buffer_pool);
    let logical = sql_to_logical_plan("SELECT * FROM users WHERE id = 7").unwrap();
    let root = PhysicalPlanner::new(&catalog).plan(&logical).unwrap();

    let operator = unwrap_projection(&root);
    let filter = operator
        .as_any()
        .downcast_ref::<Filter>()
        .expect("expected filter");
    assert!(filter.child().as_any().is::<SeqScan>());
}

#[test]
fn nonsargable_predicate_not_indexscan() {
    let buffer_pool = temp_buffer_pool();
    let (mut catalog, _) = make_catalog_with_users_table(buffer_pool);
    catalog
        .table_mut("users")
        .unwrap()
        .create_index("users_pk", "id", true, true)
        .unwrap();

    let logical = sql_to_logical_plan("SELECT * FROM users WHERE id + 1 = 43").unwrap();
    let root = PhysicalPlanner::new(&catalog).plan(&logical).unwrap();
    let operator = unwrap_projection(&root);
    let filter = operator
        .as_any()
        .downcast_ref::<Filter>()
        .expect("expected filter");
    assert!(filter.child().as_any().is::<SeqScan>());
}
