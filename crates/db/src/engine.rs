use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use query::execution::operator::evaluate_expr;
use query::{
    Catalog, ColumnDef, DataType, Executor, Expr, Field, LogicalPlan, PhysicalPlanner,
    RecoveryManager, Schema, TableHeap, TableInfo, Tuple, Value, sql_to_logical_plan,
};
use serde::{Deserialize, Serialize};
use storage::{BufferPoolManager, DiskManager};
use txn::{DeadlockPolicy, LockManager};
use wal::{LogManager, TransactionManager};

use crate::printer::ReplOutput;

const DEFAULT_POOL_SIZE: usize = 64;

pub struct Engine {
    catalog: Catalog,
    buffer_pool: BufferPoolManager,
    #[allow(dead_code)]
    log_manager: Arc<LogManager>,
    #[allow(dead_code)]
    lock_manager: Arc<LockManager>,
    txn_manager: TransactionManager,
    recovery: RecoveryManager,
    #[allow(dead_code)]
    wal_path: PathBuf,
}

impl Engine {
    pub fn new(db_path: &Path) -> Result<Self> {
        Self::new_with_pool(db_path, DEFAULT_POOL_SIZE)
    }

    pub fn new_with_pool(db_path: &Path, pool_size: usize) -> Result<Self> {
        let disk_manager = DiskManager::open(db_path).context("open database file")?;
        let wal_path = db_path.with_extension("wal");
        let log_manager = Arc::new(LogManager::open(&wal_path).context("open wal file")?);
        let buffer_pool = BufferPoolManager::new_with_log(
            disk_manager,
            pool_size,
            Some(Arc::clone(&log_manager)),
        );
        let lock_manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            std::time::Duration::from_secs(1),
        )));
        let txn_manager = TransactionManager::with_lock_manager(
            Arc::clone(&log_manager),
            Arc::clone(&lock_manager),
        );
        let recovery = RecoveryManager::new(Arc::clone(&log_manager), &wal_path);
        let catalog_path = wal_path.with_extension("catalog");

        let mut engine = Self {
            catalog: Catalog::new(),
            buffer_pool,
            log_manager,
            lock_manager,
            txn_manager,
            recovery,
            wal_path,
        };

        engine.recovery.recover(&engine.buffer_pool)?;
        engine.load_catalog(&catalog_path)?;
        Ok(engine)
    }

    pub fn checkpoint(&mut self) -> Result<()> {
        self.buffer_pool
            .flush_all_pages_with_mode(storage::FlushMode::Force)
            .context("flush pages for checkpoint")?;
        self.log_manager.force_flush().context("force flush WAL")?;
        self.persist_catalog()?;
        Ok(())
    }

    fn persist_catalog(&self) -> Result<()> {
        let catalog_path = self.wal_path.with_extension("catalog");
        self._persist_catalog(&catalog_path)
    }

    pub fn begin_transaction(&mut self) -> Result<wal::TransactionHandle> {
        self.txn_manager.begin().context("begin transaction")
    }

    pub fn execute_sql_in_transaction(
        &mut self,
        sql: &str,
        txn: &wal::TransactionHandle,
    ) -> Result<ReplOutput> {
        let plan = sql_to_logical_plan(sql)?;
        let txn_manager = self.txn_manager.clone();
        txn_manager.with_transaction(txn, || self.execute_plan(plan))
    }

    pub fn commit_transaction(&mut self, txn: &wal::TransactionHandle) -> Result<()> {
        self.txn_manager.commit(txn).context("commit transaction")?;
        Ok(())
    }

    pub fn abort_transaction(&mut self, txn: &wal::TransactionHandle) -> Result<()> {
        self.txn_manager.abort(txn).context("abort transaction")?;
        self.recovery
            .rollback_transaction(&self.buffer_pool, txn)
            .context("rollback transaction")?;
        Ok(())
    }

    pub fn execute_sql(&mut self, sql: &str) -> Result<ReplOutput> {
        let plan = sql_to_logical_plan(sql)?;
        let txn = self.txn_manager.begin().context("begin transaction")?;
        let txn_manager = self.txn_manager.clone();
        let result = txn_manager.with_transaction(&txn, || self.execute_plan(plan));

        match result {
            Ok(output) => {
                self.txn_manager
                    .commit(&txn)
                    .context("commit transaction")?;
                Ok(output)
            }
            Err(error) => {
                self.txn_manager.abort(&txn).context("abort transaction")?;
                self.recovery
                    .rollback_transaction(&self.buffer_pool, &txn)
                    .context("rollback transaction")?;
                Err(error)
            }
        }
    }

    pub fn list_tables(&self) -> Vec<String> {
        self.catalog.table_names()
    }

    fn execute_plan(&mut self, plan: LogicalPlan) -> Result<ReplOutput> {
        match plan {
            LogicalPlan::CreateTable {
                table_name,
                columns,
                if_not_exists,
            } => self.create_table(&table_name, &columns, if_not_exists),
            LogicalPlan::DropTable {
                table_name,
                if_exists,
            } => self.drop_table(&table_name, if_exists),
            LogicalPlan::AlterTableRename {
                table_name,
                new_table_name,
            } => self.alter_table_rename(&table_name, &new_table_name),
            LogicalPlan::AlterTableRenameColumn {
                table_name,
                old_column_name,
                new_column_name,
            } => self.alter_table_rename_column(&table_name, &old_column_name, &new_column_name),
            LogicalPlan::AlterTableAddColumn {
                table_name,
                column_def,
            } => self.alter_table_add_column(&table_name, &column_def),
            LogicalPlan::AlterTableDropColumn {
                table_name,
                column_name,
            } => self.alter_table_drop_column(&table_name, &column_name),
            LogicalPlan::Insert {
                table_name,
                columns,
                values,
                ..
            } => self.insert_rows(&table_name, columns.as_deref(), &values),
            LogicalPlan::Delete {
                table_name, filter, ..
            } => self.delete_rows(&table_name, filter.as_ref()),
            LogicalPlan::Update { .. } => self.execute_update(plan),
            _ => self.execute_query(plan),
        }
    }

    pub fn table_schema(&self, table_name: &str) -> Option<Schema> {
        self.catalog
            .table(table_name)
            .map(|table| table.schema.visible_schema())
    }

    fn create_table(
        &mut self,
        table_name: &str,
        columns: &[ColumnDef],
        if_not_exists: bool,
    ) -> Result<ReplOutput> {
        if self.catalog.table(table_name).is_some() {
            if if_not_exists {
                return Ok(ReplOutput::Message("OK".to_string()));
            }
            bail!("table {} already exists", table_name);
        }

        let schema = Schema::new(
            columns
                .iter()
                .map(|column| Field {
                    name: column.name.clone(),
                    table: Some(table_name.to_string()),
                    data_type: column.data_type.clone(),
                    nullable: column.nullable,
                    visible: true,
                })
                .collect(),
        );

        let heap = TableHeap::create(self.buffer_pool.clone())
            .map_err(|err| anyhow!(err))
            .context("create table heap")?;
        let mut table =
            TableInfo::with_columns(table_name.to_string(), schema, columns.to_vec(), heap);
        for column in columns {
            if column.primary_key || column.unique {
                let index_name = if column.primary_key {
                    format!("{}_{}_pk", table_name, column.name)
                } else {
                    format!("{}_{}_uk", table_name, column.name)
                };
                table
                    .create_index(index_name, &column.name, true, column.primary_key)
                    .map_err(|err| anyhow!(err))?;
            }
        }
        self.catalog.register_table_info(table);
        self.persist_catalog()?;
        Ok(ReplOutput::Message("OK".to_string()))
    }

    fn drop_table(&mut self, table_name: &str, if_exists: bool) -> Result<ReplOutput> {
        match self.catalog.drop_table(table_name) {
            Ok(()) => {
                self.persist_catalog()?;
                Ok(ReplOutput::Message("OK".to_string()))
            }
            Err(_) if if_exists => Ok(ReplOutput::Message("OK".to_string())),
            Err(err) => Err(anyhow!(err)),
        }
    }

    fn alter_table_rename(&mut self, table_name: &str, new_table_name: &str) -> Result<ReplOutput> {
        self.catalog
            .rename_table(table_name, new_table_name)
            .map_err(|err| anyhow!(err))?;
        self.persist_catalog()?;
        Ok(ReplOutput::Message("OK".to_string()))
    }

    fn alter_table_rename_column(
        &mut self,
        table_name: &str,
        old_column_name: &str,
        new_column_name: &str,
    ) -> Result<ReplOutput> {
        self.catalog
            .rename_column(table_name, old_column_name, new_column_name)
            .map_err(|err| anyhow!(err))?;
        self.persist_catalog()?;
        Ok(ReplOutput::Message("OK".to_string()))
    }

    fn alter_table_add_column(
        &mut self,
        table_name: &str,
        column_def: &ColumnDef,
    ) -> Result<ReplOutput> {
        self.catalog
            .add_column(table_name, column_def.clone())
            .map_err(|err| anyhow!(err))?;
        self.persist_catalog()?;
        Ok(ReplOutput::Message("OK".to_string()))
    }

    fn alter_table_drop_column(
        &mut self,
        table_name: &str,
        column_name: &str,
    ) -> Result<ReplOutput> {
        self.catalog
            .drop_column(table_name, column_name)
            .map_err(|err| anyhow!(err))?;
        self.persist_catalog()?;
        Ok(ReplOutput::Message("OK".to_string()))
    }

    fn insert_rows(
        &mut self,
        table_name: &str,
        columns: Option<&[String]>,
        values: &[Vec<Expr>],
    ) -> Result<ReplOutput> {
        let table = self
            .catalog
            .table(table_name)
            .ok_or_else(|| anyhow!("table {} not found", table_name))?;
        let schema = &table.schema;
        let column_indices = resolve_column_indices(schema, columns)?;

        let mut inserted = 0;
        for row in values {
            if row.len() != column_indices.len() {
                bail!(
                    "expected {} values, got {}",
                    column_indices.len(),
                    row.len()
                );
            }
            let mut values = vec![Value::Null; schema.fields.len()];
            for (expr, column_index) in row.iter().zip(column_indices.iter()) {
                let value = evaluate_insert_expr(expr)?;
                values[*column_index] = value;
            }
            for (idx, field) in schema.fields.iter().enumerate() {
                if !field.visible {
                    continue;
                }
                if values[idx].is_null() && !field.nullable {
                    bail!("missing value for non-nullable column {}", field.name);
                }
            }
            let tuple = Tuple::new(values);
            table.insert_tuple(&tuple).map_err(|err| anyhow!(err))?;
            inserted += 1;
        }

        Ok(ReplOutput::Message(format!("INSERT 0 {}", inserted)))
    }

    fn delete_rows(&mut self, table_name: &str, filter: Option<&Expr>) -> Result<ReplOutput> {
        let table = self
            .catalog
            .table(table_name)
            .ok_or_else(|| anyhow!("table {} not found", table_name))?;
        let deleted = table.delete_tuples(filter).map_err(|err| anyhow!(err))?;
        Ok(ReplOutput::Message(format!("DELETE {}", deleted)))
    }

    fn execute_update(&mut self, plan: LogicalPlan) -> Result<ReplOutput> {
        let root = PhysicalPlanner::new(&self.catalog)
            .plan(&plan)
            .map_err(|err| anyhow!(err))?;
        let mut executor = Executor::new(root);
        let rows = executor.execute().map_err(|err| anyhow!(err))?;
        Ok(ReplOutput::Message(format!("UPDATE {}", rows.len())))
    }

    fn execute_query(&mut self, plan: LogicalPlan) -> Result<ReplOutput> {
        let schema = plan.schema();
        let root = PhysicalPlanner::new(&self.catalog)
            .plan(&plan)
            .map_err(|err| anyhow!(err))?;
        let mut executor = Executor::new(root);
        let rows = executor.execute().map_err(|err| anyhow!(err))?;
        Ok(ReplOutput::Rows { schema, rows })
    }

    fn _persist_catalog(&self, path: &Path) -> Result<()> {
        #[derive(Serialize)]
        struct SerializedCatalog {
            tables: Vec<SerializedTable>,
        }
        #[derive(Serialize)]
        struct SerializedTable {
            name: String,
            first_page_id: u64,
            columns: Vec<SerializedColumn>,
            indexes: Vec<SerializedIndex>,
        }
        #[derive(Serialize)]
        struct SerializedColumn {
            name: String,
            data_type: String,
            nullable: bool,
            primary_key: bool,
            unique: bool,
            default_value: Option<SerializedDefaultValue>,
        }

        #[derive(Serialize, Clone)]
        enum SerializedDefaultValue {
            Null,
            Integer(i64),
            Real(f64),
            Text(String),
            Boolean(bool),
            CurrentTimestamp,
        }

        impl From<query::DefaultValue> for SerializedDefaultValue {
            fn from(default: query::DefaultValue) -> Self {
                match default {
                    query::DefaultValue::Null => SerializedDefaultValue::Null,
                    query::DefaultValue::Integer(v) => SerializedDefaultValue::Integer(v),
                    query::DefaultValue::Real(v) => SerializedDefaultValue::Real(v),
                    query::DefaultValue::Text(v) => SerializedDefaultValue::Text(v),
                    query::DefaultValue::Boolean(v) => SerializedDefaultValue::Boolean(v),
                    query::DefaultValue::CurrentTimestamp => {
                        SerializedDefaultValue::CurrentTimestamp
                    }
                }
            }
        }

        #[derive(Serialize)]
        struct SerializedIndex {
            name: String,
            columns: Vec<String>,
            unique: bool,
            is_primary: bool,
        }

        let mut tables = Vec::new();
        for table in self.catalog.tables() {
            let columns: Vec<SerializedColumn> = table
                .columns
                .iter()
                .map(|c| SerializedColumn {
                    name: c.name.clone(),
                    data_type: format!("{:?}", c.data_type),
                    nullable: c.nullable,
                    primary_key: c.primary_key,
                    unique: c.unique,
                    default_value: c.default_value.as_ref().map(|v| v.clone().into()),
                })
                .collect();

            let first_page_id = table.heap.first_page_id().unwrap_or(None).unwrap_or(0);

            let indexes: Vec<SerializedIndex> = table
                .indexes
                .iter()
                .map(|idx| SerializedIndex {
                    name: idx.name.clone(),
                    columns: idx.columns.clone(),
                    unique: idx.unique,
                    is_primary: idx.is_primary,
                })
                .collect();

            tables.push(SerializedTable {
                name: table.name.clone(),
                first_page_id,
                columns,
                indexes,
            });
        }

        let catalog_data = SerializedCatalog { tables };
        let file = File::create(path).context("create catalog file")?;
        serde_json::to_writer_pretty(file, &catalog_data)?;
        Ok(())
    }

    fn load_catalog(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        #[derive(Deserialize)]
        struct SerializedCatalog {
            tables: Vec<SerializedTable>,
        }
        #[derive(Deserialize)]
        struct SerializedTable {
            name: String,
            first_page_id: u64,
            columns: Vec<SerializedColumn>,
            indexes: Vec<SerializedIndex>,
        }
        #[derive(Deserialize)]
        struct SerializedColumn {
            name: String,
            data_type: String,
            nullable: bool,
            primary_key: bool,
            unique: bool,
            default_value: Option<SerializedDefaultValue>,
        }

        #[derive(Deserialize, Clone)]
        enum SerializedDefaultValue {
            Null,
            Integer(i64),
            Real(f64),
            Text(String),
            Boolean(bool),
            CurrentTimestamp,
        }

        impl From<SerializedDefaultValue> for query::DefaultValue {
            fn from(default: SerializedDefaultValue) -> Self {
                match default {
                    SerializedDefaultValue::Null => query::DefaultValue::Null,
                    SerializedDefaultValue::Integer(v) => query::DefaultValue::Integer(v),
                    SerializedDefaultValue::Real(v) => query::DefaultValue::Real(v),
                    SerializedDefaultValue::Text(v) => query::DefaultValue::Text(v),
                    SerializedDefaultValue::Boolean(v) => query::DefaultValue::Boolean(v),
                    SerializedDefaultValue::CurrentTimestamp => {
                        query::DefaultValue::CurrentTimestamp
                    }
                }
            }
        }

        #[derive(Deserialize)]
        struct SerializedIndex {
            name: String,
            columns: Vec<String>,
            unique: bool,
            is_primary: bool,
        }

        let file = File::open(path).context("open catalog file")?;
        let catalog_data: SerializedCatalog =
            serde_json::from_reader(file).context("parse catalog")?;

        for table_data in catalog_data.tables {
            let columns: Result<Vec<ColumnDef>, _> = table_data
                .columns
                .iter()
                .map(|c| {
                    let data_type = match c.data_type.as_str() {
                        "Integer" => DataType::Integer,
                        "BigInt" => DataType::BigInt,
                        "Text" => DataType::Text,
                        "Boolean" => DataType::Boolean,
                        "Real" => DataType::Real,
                        "Timestamp" => DataType::Timestamp,
                        "Blob" => DataType::Blob,
                        _ => {
                            return Err(anyhow!(
                                "unknown data type '{}' for column '{}' in table '{}'",
                                c.data_type,
                                c.name,
                                table_data.name
                            ));
                        }
                    };
                    Ok(ColumnDef {
                        name: c.name.clone(),
                        data_type,
                        nullable: c.nullable,
                        primary_key: c.primary_key,
                        unique: c.unique,
                        default_value: c.default_value.as_ref().map(|v| (*v).clone().into()),
                    })
                })
                .collect();
            let columns = columns.context("failed to parse column definitions")?;

            let schema = Schema::new(
                columns
                    .iter()
                    .map(|c| Field {
                        name: c.name.clone(),
                        table: Some(table_data.name.clone()),
                        data_type: c.data_type.clone(),
                        nullable: c.nullable,
                        visible: true,
                    })
                    .collect(),
            );

            let heap = TableHeap::load(table_data.first_page_id, self.buffer_pool.clone())
                .map_err(|e| anyhow!("failed to load table heap: {}", e))?;

            let mut table = TableInfo::with_columns(table_data.name.clone(), schema, columns, heap);

            for idx in &table_data.indexes {
                let column_names: Vec<&str> = idx.columns.iter().map(|c| c.as_str()).collect();
                table
                    .create_composite_index(
                        idx.name.clone(),
                        column_names,
                        idx.unique,
                        idx.is_primary,
                    )
                    .map_err(|e| anyhow!(e))?;
            }

            self.catalog.register_table_info(table);
        }

        Ok(())
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        use storage::FlushMode;
        let _ = self.buffer_pool.flush_all_pages_with_mode(FlushMode::Force);
        if let Err(e) = self.persist_catalog() {
            eprintln!("WARN: failed to persist catalog: {}", e);
        }
    }
}

fn resolve_column_indices(schema: &Schema, columns: Option<&[String]>) -> Result<Vec<usize>> {
    let mut indices = Vec::new();
    let mut seen = HashSet::new();
    match columns {
        Some(columns) => {
            for column in columns {
                let name = column.split('.').next_back().unwrap_or(column);
                let index = schema
                    .field_index(name)
                    .ok_or_else(|| anyhow!("column {} not found", column))?;
                if !seen.insert(index) {
                    bail!("column {} specified more than once", column);
                }
                indices.push(index);
            }
        }
        None => {
            indices.extend(
                schema
                    .fields
                    .iter()
                    .enumerate()
                    .filter_map(|(index, field)| field.visible.then_some(index)),
            );
        }
    }
    Ok(indices)
}

fn evaluate_insert_expr(expr: &Expr) -> Result<Value> {
    match expr {
        Expr::Literal(literal) => Ok(Value::from(literal)),
        _ => evaluate_expr(expr, &Tuple::new(Vec::new()), &Schema::empty())
            .map_err(|err| anyhow!(err)),
    }
}

pub fn schema_to_description(schema: &Schema) -> ReplOutput {
    let output_schema = Schema::new(vec![
        Field {
            name: "column".to_string(),
            table: None,
            data_type: DataType::Text,
            nullable: false,
            visible: true,
        },
        Field {
            name: "type".to_string(),
            table: None,
            data_type: DataType::Text,
            nullable: false,
            visible: true,
        },
        Field {
            name: "nullable".to_string(),
            table: None,
            data_type: DataType::Text,
            nullable: false,
            visible: true,
        },
    ]);

    let rows = schema
        .visible_fields()
        .map(|field| {
            let values = vec![
                Value::String(field.name.clone()),
                Value::String(format!("{:?}", field.data_type)),
                Value::String(field.nullable.to_string()),
            ];
            Tuple::new(values)
        })
        .collect::<Vec<_>>();

    ReplOutput::Rows {
        schema: output_schema,
        rows,
    }
}

pub fn tables_to_output(tables: &[String]) -> ReplOutput {
    let output_schema = Schema::new(vec![Field {
        name: "table".to_string(),
        table: None,
        data_type: DataType::Text,
        nullable: false,
        visible: true,
    }]);
    let rows = tables
        .iter()
        .map(|name| Tuple::new(vec![Value::String(name.clone())]))
        .collect();
    ReplOutput::Rows {
        schema: output_schema,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDb {
        path: PathBuf,
    }

    impl TestDb {
        fn new(test_name: &str) -> Self {
            let mut path = env::temp_dir();
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos();
            path.push(format!(
                "rdbms_engine_{}_{}_{}.db",
                test_name,
                std::process::id(),
                nanos
            ));
            Self { path }
        }
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(self.path.with_extension("wal"));
            let _ = fs::remove_file(self.path.with_extension("catalog"));
        }
    }

    #[test]
    fn insert_rollback_on_constraint_failure() {
        let db = TestDb::new("constraint");
        let mut engine = Engine::new(&db.path).expect("engine init");

        engine
            .execute_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT);")
            .expect("create table");
        engine
            .execute_sql("INSERT INTO users VALUES (1, 'Ada');")
            .expect("insert user");
        assert!(
            engine
                .execute_sql("INSERT INTO users VALUES (1, 'Ada');")
                .is_err()
        );

        let output = engine
            .execute_sql("SELECT * FROM users;")
            .expect("select users");
        match output {
            ReplOutput::Rows { rows, .. } => assert_eq!(rows.len(), 1),
            _ => panic!("expected rows output"),
        }
    }

    #[test]
    fn alter_table_sequence_updates_schema_and_rows() {
        let db = TestDb::new("alter_sequence");
        let mut engine = Engine::new(&db.path).expect("engine init");

        engine
            .execute_sql("CREATE TABLE people (id INT, tag TEXT);")
            .expect("create table");
        engine
            .execute_sql("INSERT INTO people VALUES (1, 'alpha');")
            .expect("insert row");
        engine
            .execute_sql("ALTER TABLE people RENAME TO users;")
            .expect("rename table");
        engine
            .execute_sql("ALTER TABLE users RENAME COLUMN tag TO username;")
            .expect("rename column");
        engine
            .execute_sql("ALTER TABLE users ADD COLUMN password TEXT;")
            .expect("add column");
        engine
            .execute_sql("ALTER TABLE users DROP COLUMN id;")
            .expect("drop column");

        let output = engine
            .execute_sql("SELECT username, password FROM users;")
            .expect("select users");
        match output {
            ReplOutput::Rows { rows, .. } => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].values()[0], Value::String("alpha".to_string()));
                assert_eq!(rows[0].values()[1], Value::Null);
            }
            _ => panic!("expected rows output"),
        }
    }

    #[test]
    fn alter_table_rejects_invalid_operations() {
        let db = TestDb::new("alter_invalid");
        let mut engine = Engine::new(&db.path).expect("engine init");

        engine
            .execute_sql("CREATE TABLE people (id INT PRIMARY KEY, name TEXT);")
            .expect("create table");

        assert!(
            engine
                .execute_sql("ALTER TABLE missing RENAME TO users;")
                .is_err()
        );
        assert!(
            engine
                .execute_sql("ALTER TABLE people RENAME COLUMN missing TO name2;")
                .is_err()
        );
        assert!(
            engine
                .execute_sql("ALTER TABLE people ADD COLUMN name TEXT;")
                .is_err()
        );
        assert!(
            engine
                .execute_sql("ALTER TABLE people DROP COLUMN id;")
                .is_err()
        );
        assert!(
            engine
                .execute_sql("ALTER TABLE people ADD COLUMN age INT, DROP COLUMN name;")
                .is_err()
        );
    }
}
