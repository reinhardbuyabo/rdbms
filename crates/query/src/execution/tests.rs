use super::{
    Catalog, ExecutionError, ExecutionResult, Executor, PhysicalOperator, PhysicalPlanner, SeqScan,
    TableHeap, TableInfo, Tuple, Value,
};
use crate::expr::{BinaryOperator, Expr, LiteralValue};
use crate::index::{Index, IndexKey};
use crate::logical_plan::{JoinType, LogicalPlan};
use crate::schema::{DataType, Field, Schema};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use storage::{BufferPoolManager, DiskManager};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestContext {
    path: PathBuf,
}

impl TestContext {
    fn new(test_name: &str) -> Self {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("chronos_query_{}_{}.db", test_name, id));
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
        Self { path }
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn setup_bpm(test_name: &str, pool_size: usize) -> (TestContext, BufferPoolManager) {
    let ctx = TestContext::new(test_name);
    let disk_manager = DiskManager::open(ctx.path.to_str().unwrap()).unwrap();
    let bpm = BufferPoolManager::new(disk_manager, pool_size);
    (ctx, bpm)
}

fn schema_for(table: &str, columns: Vec<(&str, DataType)>) -> Schema {
    Schema::new(
        columns
            .into_iter()
            .map(|(name, data_type)| Field {
                name: name.to_string(),
                table: Some(table.to_string()),
                data_type,
                nullable: true,
            })
            .collect(),
    )
}

fn tuples(rows: Vec<Vec<Value>>) -> Vec<Tuple> {
    rows.into_iter().map(Tuple::new).collect()
}

fn build_table(
    bpm: &BufferPoolManager,
    table: &str,
    columns: Vec<(&str, DataType)>,
    rows: Vec<Vec<Value>>,
) -> ExecutionResult<(Schema, TableHeap, Vec<Tuple>)> {
    let schema = schema_for(table, columns);
    let heap = TableHeap::create(bpm.clone())?;
    let tuples = tuples(rows);
    for tuple in &tuples {
        let _ = heap.insert_tuple(tuple, &schema)?;
    }
    Ok((schema, heap, tuples))
}

fn register_table(catalog: &mut Catalog, name: &str, schema: Schema, heap: TableHeap) {
    catalog.register_table(name.to_string(), schema, heap);
}

fn register_table_info(catalog: &mut Catalog, table: TableInfo) {
    catalog.register_table_info(table);
}

fn scan_plan(table: &str) -> LogicalPlan {
    LogicalPlan::Scan {
        table_name: table.to_string(),
        alias: None,
        schema: None,
    }
}

fn execute_plan(plan: LogicalPlan, catalog: &Catalog) -> ExecutionResult<Vec<Tuple>> {
    let planner = PhysicalPlanner::new(catalog);
    let operator = planner.plan(&plan)?;
    let mut executor = Executor::new(operator);
    executor.execute()
}

fn execute_twice(
    plan: &LogicalPlan,
    catalog: &Catalog,
) -> ExecutionResult<(Vec<Tuple>, Vec<Tuple>)> {
    let first = execute_plan(plan.clone(), catalog)?;
    let second = execute_plan(plan.clone(), catalog)?;
    Ok((first, second))
}

fn assert_deterministic(plan: &LogicalPlan, catalog: &Catalog) -> ExecutionResult<Vec<Tuple>> {
    let (first, second) = execute_twice(plan, catalog)?;
    assert_eq!(first, second);
    Ok(first)
}

fn col(table: &str, name: &str) -> Expr {
    Expr::Column {
        table: Some(table.to_string()),
        name: name.to_string(),
    }
}

fn lit_int(value: i64) -> Expr {
    Expr::Literal(LiteralValue::Integer(value))
}

fn lit_bool(value: bool) -> Expr {
    Expr::Literal(LiteralValue::Boolean(value))
}

fn bin(left: Expr, op: BinaryOperator, right: Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(left),
        op,
        right: Box::new(right),
    }
}

#[test]
fn seq_scan_lifecycle_happy_path() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("seq_scan_lifecycle", 8);
    let (schema, heap, rows) = build_table(
        &bpm,
        "people",
        vec![("id", DataType::Integer)],
        vec![vec![Value::Integer(1)], vec![Value::Integer(2)]],
    )?;
    let mut scan = SeqScan::new(heap, schema);
    scan.open()?;
    assert_eq!(scan.next()?, Some(rows[0].clone()));
    assert_eq!(scan.next()?, Some(rows[1].clone()));
    assert_eq!(scan.next()?, None);
    assert_eq!(scan.next()?, None);
    scan.close()?;
    Ok(())
}

#[test]
fn seq_scan_next_before_open_is_safe() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("seq_scan_next_before_open", 8);
    let (schema, heap, rows) = build_table(
        &bpm,
        "people",
        vec![("id", DataType::Integer)],
        vec![vec![Value::Integer(10)], vec![Value::Integer(20)]],
    )?;
    let mut scan = SeqScan::new(heap, schema);
    assert_eq!(scan.next()?, None);
    scan.open()?;
    assert_eq!(scan.next()?, Some(rows[0].clone()));
    scan.close()?;
    Ok(())
}

#[test]
fn seq_scan_close_is_idempotent() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("seq_scan_close_idempotent", 8);
    let (schema, heap, _rows) = build_table(
        &bpm,
        "people",
        vec![("id", DataType::Integer)],
        vec![vec![Value::Integer(1)]],
    )?;
    let mut scan = SeqScan::new(heap, schema);
    scan.open()?;
    scan.close()?;
    scan.close()?;
    Ok(())
}

#[test]
fn tuples_outlive_executor() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("tuple_outlive", 8);
    let (schema, heap, expected) = build_table(
        &bpm,
        "people",
        vec![("id", DataType::Integer), ("name", DataType::Text)],
        vec![
            vec![Value::Integer(1), Value::String("Ada".to_string())],
            vec![Value::Integer(2), Value::String("Linus".to_string())],
        ],
    )?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "people", schema, heap);
    let plan = scan_plan("people");
    let results = execute_plan(plan, &catalog)?;
    assert_eq!(results, expected);
    assert!(matches!(results[0].get(1), Some(Value::String(name)) if name == "Ada"));
    Ok(())
}

#[test]
fn seq_scan_empty_table() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("seq_scan_empty", 4);
    let schema = schema_for("empty", vec![("id", DataType::Integer)]);
    let heap = TableHeap::create(bpm.clone())?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "empty", schema, heap);
    let plan = scan_plan("empty");
    let results = execute_plan(plan, &catalog)?;
    assert!(results.is_empty());
    Ok(())
}

#[test]
fn seq_scan_multi_page() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("seq_scan_multi_page", 6);
    let rows: Vec<Vec<Value>> = (0..400).map(|i| vec![Value::Integer(i)]).collect();
    let (schema, heap, expected) =
        build_table(&bpm, "numbers", vec![("id", DataType::Integer)], rows)?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "numbers", schema, heap);
    let plan = scan_plan("numbers");
    let results = execute_plan(plan, &catalog)?;
    assert_eq!(results.len(), expected.len());
    assert_eq!(results.first(), expected.first());
    assert_eq!(results.last(), expected.last());
    Ok(())
}

#[test]
fn seq_scan_buffer_pool_pressure_is_deterministic() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("seq_scan_pressure", 2);
    let rows: Vec<Vec<Value>> = (0..512).map(|i| vec![Value::Integer(i)]).collect();
    let (schema, heap, _expected) =
        build_table(&bpm, "numbers", vec![("id", DataType::Integer)], rows)?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "numbers", schema, heap);
    let plan = scan_plan("numbers");
    let results = assert_deterministic(&plan, &catalog)?;
    assert_eq!(results.len(), 512);
    Ok(())
}

#[test]
fn filter_selects_subset() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("filter_subset", 6);
    let rows: Vec<Vec<Value>> = (1..=5).map(|i| vec![Value::Integer(i)]).collect();
    let (schema, heap, _expected) =
        build_table(&bpm, "numbers", vec![("id", DataType::Integer)], rows)?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "numbers", schema.clone(), heap);
    let predicate = bin(col("numbers", "id"), BinaryOperator::Gt, lit_int(3));
    let plan = LogicalPlan::Filter {
        input: Box::new(scan_plan("numbers")),
        predicate,
    };
    let results = execute_plan(plan, &catalog)?;
    let expected = vec![
        Tuple::new(vec![Value::Integer(4)]),
        Tuple::new(vec![Value::Integer(5)]),
    ];
    assert_eq!(results, expected);
    Ok(())
}

#[test]
fn filter_empty_input() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("filter_empty", 4);
    let schema = schema_for("empty", vec![("id", DataType::Integer)]);
    let heap = TableHeap::create(bpm.clone())?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "empty", schema, heap);
    let predicate = lit_bool(true);
    let plan = LogicalPlan::Filter {
        input: Box::new(scan_plan("empty")),
        predicate,
    };
    let results = execute_plan(plan, &catalog)?;
    assert!(results.is_empty());
    Ok(())
}

#[test]
fn projection_subset_and_reorder_materializes() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("projection_subset", 6);
    let (schema, heap, _expected) = build_table(
        &bpm,
        "people",
        vec![
            ("id", DataType::Integer),
            ("name", DataType::Text),
            ("age", DataType::Integer),
        ],
        vec![vec![
            Value::Integer(1),
            Value::String("Ada".to_string()),
            Value::Integer(42),
        ]],
    )?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "people", schema.clone(), heap);
    let plan = LogicalPlan::Project {
        input: Box::new(scan_plan("people")),
        expressions: vec![col("people", "age"), col("people", "name")],
        aliases: None,
    };
    let results = execute_plan(plan, &catalog)?;
    assert_eq!(
        results,
        vec![Tuple::new(vec![
            Value::Integer(42),
            Value::String("Ada".to_string())
        ])]
    );
    assert!(matches!(results[0].get(1), Some(Value::String(name)) if name == "Ada"));
    Ok(())
}

#[test]
fn join_one_to_many_and_ordering() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("join_one_to_many", 8);
    let (event_schema, event_heap, _event_rows) = build_table(
        &bpm,
        "event",
        vec![("id", DataType::Integer), ("title", DataType::Text)],
        vec![
            vec![Value::Integer(1), Value::String("Concert".to_string())],
            vec![Value::Integer(2), Value::String("Talk".to_string())],
        ],
    )?;
    let (ticket_schema, ticket_heap, _ticket_rows) = build_table(
        &bpm,
        "ticket",
        vec![("id", DataType::Integer), ("event_id", DataType::Integer)],
        vec![
            vec![Value::Integer(10), Value::Integer(1)],
            vec![Value::Integer(11), Value::Integer(1)],
            vec![Value::Integer(12), Value::Integer(2)],
        ],
    )?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "event", event_schema, event_heap);
    register_table(&mut catalog, "ticket", ticket_schema, ticket_heap);

    let predicate = bin(
        col("event", "id"),
        BinaryOperator::Eq,
        col("ticket", "event_id"),
    );
    let plan = LogicalPlan::Join {
        left: Box::new(scan_plan("event")),
        right: Box::new(scan_plan("ticket")),
        join_type: JoinType::Inner,
        condition: Some(predicate),
    };
    let results = execute_plan(plan, &catalog)?;
    let expected = vec![
        Tuple::new(vec![
            Value::Integer(1),
            Value::String("Concert".to_string()),
            Value::Integer(10),
            Value::Integer(1),
        ]),
        Tuple::new(vec![
            Value::Integer(1),
            Value::String("Concert".to_string()),
            Value::Integer(11),
            Value::Integer(1),
        ]),
        Tuple::new(vec![
            Value::Integer(2),
            Value::String("Talk".to_string()),
            Value::Integer(12),
            Value::Integer(2),
        ]),
    ];
    assert_eq!(results, expected);
    Ok(())
}

#[test]
fn join_inner_rewind_handles_late_matches() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("join_rewind", 6);
    let (left_schema, left_heap, _left_rows) = build_table(
        &bpm,
        "left",
        vec![("id", DataType::Integer)],
        vec![
            vec![Value::Integer(1)],
            vec![Value::Integer(2)],
            vec![Value::Integer(3)],
        ],
    )?;
    let (right_schema, right_heap, _right_rows) = build_table(
        &bpm,
        "right",
        vec![("id", DataType::Integer)],
        vec![vec![Value::Integer(3)]],
    )?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "left", left_schema, left_heap);
    register_table(&mut catalog, "right", right_schema, right_heap);

    let predicate = bin(col("left", "id"), BinaryOperator::Eq, col("right", "id"));
    let plan = LogicalPlan::Join {
        left: Box::new(scan_plan("left")),
        right: Box::new(scan_plan("right")),
        join_type: JoinType::Inner,
        condition: Some(predicate),
    };
    let results = execute_plan(plan, &catalog)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get(0), Some(&Value::Integer(3)));
    Ok(())
}

#[test]
fn acceptance_query_projection_join() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("acceptance_query", 8);
    let (event_schema, event_heap, _event_rows) = build_table(
        &bpm,
        "event",
        vec![
            ("id", DataType::Integer),
            ("title", DataType::Text),
            ("venue_id", DataType::Integer),
        ],
        vec![
            vec![
                Value::Integer(1),
                Value::String("Concert".to_string()),
                Value::Integer(10),
            ],
            vec![
                Value::Integer(2),
                Value::String("Talk".to_string()),
                Value::Integer(20),
            ],
        ],
    )?;
    let (ticket_schema, ticket_heap, _ticket_rows) = build_table(
        &bpm,
        "ticket_type",
        vec![
            ("id", DataType::Integer),
            ("event_id", DataType::Integer),
            ("price", DataType::Integer),
        ],
        vec![
            vec![Value::Integer(100), Value::Integer(1), Value::Integer(50)],
            vec![Value::Integer(101), Value::Integer(1), Value::Integer(70)],
            vec![Value::Integer(102), Value::Integer(2), Value::Integer(40)],
        ],
    )?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "event", event_schema, event_heap);
    register_table(&mut catalog, "ticket_type", ticket_schema, ticket_heap);

    let join_predicate = bin(
        col("event", "id"),
        BinaryOperator::Eq,
        col("ticket_type", "event_id"),
    );
    let join_plan = LogicalPlan::Join {
        left: Box::new(scan_plan("event")),
        right: Box::new(scan_plan("ticket_type")),
        join_type: JoinType::Inner,
        condition: Some(join_predicate),
    };
    let plan = LogicalPlan::Project {
        input: Box::new(join_plan),
        expressions: vec![col("event", "title"), col("ticket_type", "price")],
        aliases: None,
    };

    let results = execute_plan(plan, &catalog)?;
    let expected = vec![
        Tuple::new(vec![
            Value::String("Concert".to_string()),
            Value::Integer(50),
        ]),
        Tuple::new(vec![
            Value::String("Concert".to_string()),
            Value::Integer(70),
        ]),
        Tuple::new(vec![Value::String("Talk".to_string()), Value::Integer(40)]),
    ];
    assert_eq!(results, expected);
    Ok(())
}

#[test]
fn deep_join_pipeline_is_deterministic() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("deep_join", 8);
    let (event_schema, event_heap, _event_rows) = build_table(
        &bpm,
        "event",
        vec![("id", DataType::Integer), ("venue_id", DataType::Integer)],
        vec![
            vec![Value::Integer(1), Value::Integer(10)],
            vec![Value::Integer(2), Value::Integer(11)],
        ],
    )?;
    let (ticket_schema, ticket_heap, _ticket_rows) = build_table(
        &bpm,
        "ticket",
        vec![("id", DataType::Integer), ("event_id", DataType::Integer)],
        vec![
            vec![Value::Integer(100), Value::Integer(1)],
            vec![Value::Integer(101), Value::Integer(2)],
        ],
    )?;
    let (venue_schema, venue_heap, _venue_rows) = build_table(
        &bpm,
        "venue",
        vec![("id", DataType::Integer)],
        vec![vec![Value::Integer(10)], vec![Value::Integer(11)]],
    )?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "event", event_schema, event_heap);
    register_table(&mut catalog, "ticket", ticket_schema, ticket_heap);
    register_table(&mut catalog, "venue", venue_schema, venue_heap);

    let event_ticket = LogicalPlan::Join {
        left: Box::new(scan_plan("event")),
        right: Box::new(scan_plan("ticket")),
        join_type: JoinType::Inner,
        condition: Some(bin(
            col("event", "id"),
            BinaryOperator::Eq,
            col("ticket", "event_id"),
        )),
    };
    let full_join = LogicalPlan::Join {
        left: Box::new(event_ticket),
        right: Box::new(scan_plan("venue")),
        join_type: JoinType::Inner,
        condition: Some(bin(
            col("event", "venue_id"),
            BinaryOperator::Eq,
            col("venue", "id"),
        )),
    };
    let results = assert_deterministic(&full_join, &catalog)?;
    assert_eq!(results.len(), 2);
    Ok(())
}

#[test]
fn determinism_guard_for_plan() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("determinism_guard", 4);
    let rows: Vec<Vec<Value>> = (0..50).map(|i| vec![Value::Integer(i)]).collect();
    let (schema, heap, _rows) =
        build_table(&bpm, "numbers", vec![("id", DataType::Integer)], rows)?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "numbers", schema, heap);
    let plan = LogicalPlan::Filter {
        input: Box::new(scan_plan("numbers")),
        predicate: bin(col("numbers", "id"), BinaryOperator::GtEq, lit_int(25)),
    };
    let results = assert_deterministic(&plan, &catalog)?;
    assert_eq!(results.len(), 25);
    Ok(())
}

#[test]
fn schema_mismatch_errors_are_typed() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("schema_mismatch", 4);
    let (schema, heap, _rows) = build_table(
        &bpm,
        "people",
        vec![("id", DataType::Integer)],
        vec![vec![Value::Integer(1)]],
    )?;
    let mut catalog = Catalog::new();
    register_table(&mut catalog, "people", schema, heap);

    let plan = LogicalPlan::Filter {
        input: Box::new(scan_plan("people")),
        predicate: bin(col("people", "missing"), BinaryOperator::Eq, lit_int(1)),
    };
    let result = execute_plan(plan, &catalog);
    match result {
        Err(ExecutionError::Schema(_)) => Ok(()),
        other => Err(ExecutionError::Execution(format!(
            "expected schema error, got {:?}",
            other
        ))),
    }
}

#[test]
fn primary_key_violation_rejects_duplicate() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("pk_violation", 8);
    let schema = schema_for(
        "people",
        vec![("id", DataType::Integer), ("name", DataType::Text)],
    );
    let heap = TableHeap::create(bpm.clone())?;
    let mut table = TableInfo::new("people", schema.clone(), heap.clone());
    table.create_index("people_pk", "id", true, true)?;
    let mut catalog = Catalog::new();
    register_table_info(&mut catalog, table);

    let tuple = Tuple::new(vec![Value::Integer(1), Value::String("Ada".to_string())]);
    catalog.insert_tuple("people", &tuple)?;
    let result = catalog.insert_tuple("people", &tuple);
    match result {
        Err(ExecutionError::ConstraintViolation {
            table,
            constraint,
            key,
        }) => {
            assert_eq!(table, "people");
            assert_eq!(constraint, "people_pk");
            assert_eq!(key, "1");
        }
        other => {
            return Err(ExecutionError::Execution(format!(
                "expected constraint violation, got {:?}",
                other
            )));
        }
    }

    let results = execute_plan(scan_plan("people"), &catalog)?;
    assert_eq!(results.len(), 1);
    Ok(())
}

#[test]
fn unique_index_rejects_duplicate() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("unique_violation", 8);
    let schema = schema_for(
        "users",
        vec![("id", DataType::Integer), ("email", DataType::Text)],
    );
    let heap = TableHeap::create(bpm.clone())?;
    let mut table = TableInfo::new("users", schema.clone(), heap.clone());
    table.create_index("users_email_unique", "email", true, false)?;
    let mut catalog = Catalog::new();
    register_table_info(&mut catalog, table);

    let first = Tuple::new(vec![
        Value::Integer(1),
        Value::String("a@chronos.dev".to_string()),
    ]);
    let second = Tuple::new(vec![
        Value::Integer(2),
        Value::String("a@chronos.dev".to_string()),
    ]);
    catalog.insert_tuple("users", &first)?;
    let result = catalog.insert_tuple("users", &second);
    assert!(matches!(
        result,
        Err(ExecutionError::ConstraintViolation { .. })
    ));
    Ok(())
}

#[test]
fn index_lookup_returns_rid() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("index_lookup", 8);
    let schema = schema_for("people", vec![("id", DataType::Integer)]);
    let heap = TableHeap::create(bpm.clone())?;
    let mut table = TableInfo::new("people", schema.clone(), heap.clone());
    table.create_index("people_pk", "id", true, true)?;
    let mut catalog = Catalog::new();
    register_table_info(&mut catalog, table);

    let tuple = Tuple::new(vec![Value::Integer(7)]);
    catalog.insert_tuple("people", &tuple)?;

    let table = catalog.table("people").unwrap();
    let index = &table.indexes[0];
    let rids = index.index.get(&IndexKey::Integer(7))?;
    assert_eq!(rids.len(), 1);
    let fetched = table.heap.get_tuple(rids[0], &table.schema)?;
    assert_eq!(fetched, Some(tuple));
    Ok(())
}

#[test]
fn index_rebuild_preserves_constraints() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("index_rebuild", 8);
    let schema = schema_for("people", vec![("id", DataType::Integer)]);
    let heap = TableHeap::create(bpm.clone())?;
    let mut table = TableInfo::new("people", schema.clone(), heap.clone());
    table.create_index("people_pk", "id", true, true)?;
    let mut catalog = Catalog::new();
    register_table_info(&mut catalog, table);

    let tuple = Tuple::new(vec![Value::Integer(5)]);
    catalog.insert_tuple("people", &tuple)?;
    catalog.table_mut("people").unwrap().rebuild_indexes()?;
    let result = catalog.insert_tuple("people", &tuple);
    assert!(matches!(
        result,
        Err(ExecutionError::ConstraintViolation { .. })
    ));
    Ok(())
}

#[test]
fn index_scan_equality_returns_tuple() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("index_scan_eq", 8);
    let rows: Vec<Vec<Value>> = (0..10).map(|i| vec![Value::Integer(i)]).collect();
    let (schema, heap, _rows) =
        build_table(&bpm, "numbers", vec![("id", DataType::Integer)], rows)?;
    let mut table = TableInfo::new("numbers", schema.clone(), heap.clone());
    table.create_index("numbers_idx", "id", true, false)?;
    let mut catalog = Catalog::new();
    register_table_info(&mut catalog, table);

    let predicate = bin(col("numbers", "id"), BinaryOperator::Eq, lit_int(4));
    let plan = LogicalPlan::Filter {
        input: Box::new(scan_plan("numbers")),
        predicate,
    };
    let results = execute_plan(plan, &catalog)?;
    assert_eq!(results, vec![Tuple::new(vec![Value::Integer(4)])]);
    Ok(())
}

#[test]
fn index_scan_projection_and_join() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("index_scan_join", 10);
    let (cust_schema, cust_heap, _cust_rows) = build_table(
        &bpm,
        "customer",
        vec![("id", DataType::Integer), ("name", DataType::Text)],
        vec![
            vec![Value::Integer(1), Value::String("Ada".to_string())],
            vec![Value::Integer(2), Value::String("Linus".to_string())],
        ],
    )?;
    let (order_schema, order_heap, _order_rows) = build_table(
        &bpm,
        "orders",
        vec![
            ("id", DataType::Integer),
            ("customer_id", DataType::Integer),
        ],
        vec![
            vec![Value::Integer(10), Value::Integer(1)],
            vec![Value::Integer(11), Value::Integer(2)],
        ],
    )?;

    let mut customer_table = TableInfo::new("customer", cust_schema.clone(), cust_heap.clone());
    customer_table.create_index("customer_idx", "id", true, true)?;

    let mut catalog = Catalog::new();
    register_table_info(&mut catalog, customer_table);
    register_table(&mut catalog, "orders", order_schema, order_heap);

    let filter = LogicalPlan::Filter {
        input: Box::new(scan_plan("customer")),
        predicate: bin(col("customer", "id"), BinaryOperator::Eq, lit_int(2)),
    };
    let join = LogicalPlan::Join {
        left: Box::new(filter),
        right: Box::new(scan_plan("orders")),
        join_type: JoinType::Inner,
        condition: Some(bin(
            col("customer", "id"),
            BinaryOperator::Eq,
            col("orders", "customer_id"),
        )),
    };
    let plan = LogicalPlan::Project {
        input: Box::new(join),
        expressions: vec![col("customer", "name"), col("orders", "id")],
        aliases: None,
    };

    let results = execute_plan(plan, &catalog)?;
    assert_eq!(
        results,
        vec![Tuple::new(vec![
            Value::String("Linus".to_string()),
            Value::Integer(11)
        ])]
    );
    Ok(())
}

#[test]
fn index_scan_touches_fewer_pages_than_seq_scan() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("index_perf", 16);
    let schema = schema_for("numbers", vec![("id", DataType::Integer)]);
    let heap = TableHeap::create(bpm.clone())?;
    for value in 0..10_000 {
        let tuple = Tuple::new(vec![Value::Integer(value)]);
        let _ = heap.insert_tuple(&tuple, &schema)?;
    }

    let mut indexed_table = TableInfo::new("numbers", schema.clone(), heap.clone());
    indexed_table.create_index("numbers_idx", "id", true, false)?;

    let mut catalog_index = Catalog::new();
    register_table_info(&mut catalog_index, indexed_table);

    let mut catalog_seq = Catalog::new();
    register_table(&mut catalog_seq, "numbers", schema.clone(), heap.clone());

    let predicate = bin(col("numbers", "id"), BinaryOperator::Eq, lit_int(9_999));
    let plan = LogicalPlan::Filter {
        input: Box::new(scan_plan("numbers")),
        predicate,
    };

    bpm.reset_fetch_count();
    let indexed_results = execute_plan(plan.clone(), &catalog_index)?;
    let indexed_fetches = bpm.fetch_count();

    bpm.reset_fetch_count();
    let seq_results = execute_plan(plan, &catalog_seq)?;
    let seq_fetches = bpm.fetch_count();

    assert_eq!(indexed_results, seq_results);
    assert!(indexed_fetches * 5 < seq_fetches);
    Ok(())
}
