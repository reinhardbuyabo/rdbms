use crate::execution::filter::Filter;
use crate::execution::index_scan::{IndexPredicate, IndexScan};
use crate::execution::nested_loop_join::NestedLoopJoin;
use crate::execution::operator::{
    evaluate_expr, evaluate_predicate, ExecutionError, ExecutionResult, PhysicalOperator,
};
use crate::execution::projection::Projection;
use crate::execution::seq_scan::{Rid, SeqScan, TableHeap};
use crate::execution::tuple::{Tuple, Value};
use crate::execution::update::Update;
use crate::expr::{BinaryOperator, Expr};
use crate::index::{BPlusTree, Index, IndexKey, IndexKeyType};
use crate::logical_plan::{Assignment, JoinType, LogicalPlan};
use crate::schema::{ColumnDef, DataType, Field, Schema};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub column_indices: Vec<usize>,
    pub key_types: Vec<IndexKeyType>,
    pub unique: bool,
    pub is_primary: bool,
    pub index: BPlusTree,
}

#[derive(Clone)]
pub struct TableInfo {
    pub name: String,
    pub schema: Schema,
    pub columns: Vec<ColumnDef>,
    pub heap: TableHeap,
    pub indexes: Vec<IndexInfo>,
    pub auto_increment_counter: Arc<Mutex<i64>>,
}

impl TableInfo {
    pub fn new(name: impl Into<String>, schema: Schema, heap: TableHeap) -> Self {
        let columns: Vec<ColumnDef> = schema
            .fields
            .iter()
            .map(|f| ColumnDef {
                name: f.name.clone(),
                data_type: f.data_type.clone(),
                nullable: f.nullable,
                primary_key: false,
                unique: false,
                default_value: None,
                auto_increment: false,
            })
            .collect();
        Self {
            name: name.into(),
            schema,
            columns,
            heap,
            indexes: Vec::new(),
            auto_increment_counter: Arc::new(Mutex::new(0)),
        }
    }

    pub fn with_columns(
        name: impl Into<String>,
        schema: Schema,
        columns: Vec<ColumnDef>,
        heap: TableHeap,
    ) -> Self {
        Self {
            name: name.into(),
            schema,
            columns,
            heap,
            indexes: Vec::new(),
            auto_increment_counter: Arc::new(Mutex::new(0)),
        }
    }

    pub fn create_index(
        &mut self,
        name: impl Into<String>,
        column: &str,
        unique: bool,
        is_primary: bool,
    ) -> ExecutionResult<()> {
        self.create_composite_index(name, vec![column], unique, is_primary)
    }

    pub fn create_composite_index(
        &mut self,
        name: impl Into<String>,
        columns: Vec<&str>,
        unique: bool,
        is_primary: bool,
    ) -> ExecutionResult<()> {
        if columns.is_empty() {
            return Err(ExecutionError::Schema(
                "index must include at least one column".to_string(),
            ));
        }
        let mut column_indices = Vec::with_capacity(columns.len());
        let mut column_names = Vec::with_capacity(columns.len());
        let mut key_types = Vec::with_capacity(columns.len());
        for column in columns {
            let column_index = self
                .schema
                .field_index(column)
                .ok_or_else(|| ExecutionError::Schema(format!("column {} not found", column)))?;
            column_indices.push(column_index);
            column_names.push(column.to_string());
            key_types.push(index_key_type_for_data_type(
                &self.schema.fields[column_index].data_type,
            )?);
        }

        let index = if key_types.len() > 1 {
            BPlusTree::create_composite(
                self.heap.buffer_pool().clone(),
                key_types.clone(),
                None,
                unique,
            )?
        } else {
            BPlusTree::create(self.heap.buffer_pool().clone(), key_types[0], None, unique)?
        };

        for (rid, tuple) in self.heap.scan_tuples(&self.schema)? {
            let key = Self::key_from_tuple(&tuple, &column_indices, &key_types)?;
            index.insert(key, rid)?;
        }

        self.indexes.push(IndexInfo {
            name: name.into(),
            columns: column_names,
            column_indices,
            key_types,
            unique,
            is_primary,
            index,
        });
        Ok(())
    }

    pub fn add_index(&mut self, index: IndexInfo) {
        self.indexes.push(index);
    }

    pub fn rename_table(&mut self, new_name: &str) {
        self.name = new_name.to_string();
        for field in &mut self.schema.fields {
            if field.table.is_some() {
                field.table = Some(new_name.to_string());
            }
        }
    }

    pub fn rename_column(&mut self, old_name: &str, new_name: &str) -> ExecutionResult<()> {
        if old_name.eq_ignore_ascii_case(new_name) {
            return Err(ExecutionError::Schema(format!(
                "column {} already has that name",
                old_name
            )));
        }
        let column_index = self
            .schema
            .fields
            .iter()
            .position(|field| field.visible && field.name.eq_ignore_ascii_case(old_name))
            .ok_or_else(|| ExecutionError::Schema(format!("column {} not found", old_name)))?;
        if self
            .schema
            .fields
            .iter()
            .any(|field| field.name.eq_ignore_ascii_case(new_name))
        {
            return Err(ExecutionError::Schema(format!(
                "column {} already exists",
                new_name
            )));
        }
        self.schema.fields[column_index].name = new_name.to_string();
        for col in &mut self.columns {
            if col.name.eq_ignore_ascii_case(old_name) {
                col.name = new_name.to_string();
                break;
            }
        }
        for index in &mut self.indexes {
            for column in &mut index.columns {
                if column.eq_ignore_ascii_case(old_name) {
                    *column = new_name.to_string();
                }
            }
        }
        Ok(())
    }

    pub fn add_column(&mut self, column_def: ColumnDef) -> ExecutionResult<()> {
        if self
            .schema
            .fields
            .iter()
            .any(|field| field.name.eq_ignore_ascii_case(&column_def.name))
        {
            return Err(ExecutionError::Schema(format!(
                "column {} already exists",
                column_def.name
            )));
        }
        if !column_def.nullable {
            return Err(ExecutionError::Schema(format!(
                "cannot add non-nullable column {}",
                column_def.name
            )));
        }
        if column_def.primary_key || column_def.unique {
            return Err(ExecutionError::Schema(
                "ALTER TABLE ADD COLUMN does not support UNIQUE or PRIMARY KEY constraints"
                    .to_string(),
            ));
        }
        self.schema.fields.push(Field {
            name: column_def.name.clone(),
            table: Some(self.name.clone()),
            data_type: column_def.data_type.clone(),
            nullable: column_def.nullable,
            visible: true,
        });
        self.columns.push(column_def);
        Ok(())
    }

    pub fn drop_column(&mut self, column_name: &str) -> ExecutionResult<()> {
        let column_index = self
            .schema
            .fields
            .iter()
            .position(|field| field.visible && field.name.eq_ignore_ascii_case(column_name))
            .ok_or_else(|| ExecutionError::Schema(format!("column {} not found", column_name)))?;
        if self.indexes.iter().any(|index| {
            index.is_primary
                && index
                    .columns
                    .iter()
                    .any(|col| col.eq_ignore_ascii_case(column_name))
        }) {
            return Err(ExecutionError::Schema(format!(
                "cannot drop primary key column {}",
                column_name
            )));
        }
        self.schema.fields[column_index].visible = false;
        if let Some(col_idx) = self
            .columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(column_name))
        {
            self.columns.remove(col_idx);
        }
        self.indexes.retain(|index| {
            !index
                .columns
                .iter()
                .any(|col| col.eq_ignore_ascii_case(column_name))
        });
        Ok(())
    }

    pub fn index_for_column(&self, column: &str) -> Option<&IndexInfo> {
        self.indexes
            .iter()
            .find(|index| index.columns.len() == 1 && index.columns[0].eq_ignore_ascii_case(column))
    }

    pub fn insert_tuple(&self, tuple: &Tuple) -> ExecutionResult<Rid> {
        let mut tuple_with_autoinc: Vec<Value> = tuple.values().to_vec();

        for (idx, column) in self.columns.iter().enumerate() {
            if column.auto_increment {
                if tuple_with_autoinc[idx].is_null() {
                    let mut counter = self.auto_increment_counter.lock();
                    *counter += 1;
                    tuple_with_autoinc[idx] = Value::Integer(*counter);
                }
            }
        }

        let new_tuple = Tuple::new(tuple_with_autoinc);

        let mut keys = Vec::with_capacity(self.indexes.len());
        for (idx, index) in self.indexes.iter().enumerate() {
            let key = Self::key_from_tuple(&new_tuple, &index.column_indices, &index.key_types)?;
            if index.unique && !index.index.get(&key)?.is_empty() {
                return Err(ExecutionError::ConstraintViolation {
                    table: self.name.clone(),
                    constraint: index.name.clone(),
                    key: key.display(),
                });
            }
            keys.push((idx, key));
        }

        let rid = self.heap.insert_tuple(&new_tuple, &self.schema)?;
        for (idx, key) in keys {
            if let Err(error) = self.indexes[idx].index.insert(key, rid) {
                let _ = self.heap.delete_tuple(rid);
                return Err(error);
            }
        }
        Ok(rid)
    }

    pub fn update_tuples(
        &self,
        assignments: &[Assignment],
        filter: Option<&Expr>,
    ) -> ExecutionResult<Vec<Tuple>> {
        let mut updated = Vec::new();
        let tuples = self.heap.scan_tuples(&self.schema)?;
        for (rid, tuple) in tuples {
            if let Some(predicate) = filter {
                if !evaluate_predicate(predicate, &tuple, &self.schema)? {
                    continue;
                }
            }

            let new_tuple = apply_assignments(&tuple, &self.schema, assignments)?;
            let mut old_keys = Vec::with_capacity(self.indexes.len());
            let mut new_keys = Vec::with_capacity(self.indexes.len());
            for index in &self.indexes {
                let old_key =
                    Self::key_from_tuple(&tuple, &index.column_indices, &index.key_types)?;
                let new_key =
                    Self::key_from_tuple(&new_tuple, &index.column_indices, &index.key_types)?;
                if index.unique {
                    let existing = index.index.get(&new_key)?;
                    if existing.iter().any(|existing_rid| *existing_rid != rid) {
                        return Err(ExecutionError::ConstraintViolation {
                            table: self.name.clone(),
                            constraint: index.name.clone(),
                            key: new_key.display(),
                        });
                    }
                }
                old_keys.push(old_key);
                new_keys.push(new_key);
            }

            let new_rid = self.heap.update_tuple(rid, &new_tuple, &self.schema)?;
            for (index, (old_key, new_key)) in
                self.indexes.iter().zip(old_keys.into_iter().zip(new_keys))
            {
                if new_rid == rid && old_key == new_key {
                    continue;
                }
                let _ = index.index.delete(&old_key, rid)?;
                index.index.insert(new_key, new_rid)?;
            }
            updated.push(new_tuple);
        }
        Ok(updated)
    }

    pub fn delete_tuples(&self, filter: Option<&Expr>) -> ExecutionResult<usize> {
        let mut deleted = 0;
        let tuples = self.heap.scan_tuples(&self.schema)?;
        for (rid, tuple) in tuples {
            if let Some(predicate) = filter {
                if !evaluate_predicate(predicate, &tuple, &self.schema)? {
                    continue;
                }
            }
            if !self.heap.delete_tuple(rid)? {
                continue;
            }
            for index in &self.indexes {
                let key = Self::key_from_tuple(&tuple, &index.column_indices, &index.key_types)?;
                let _ = index.index.delete(&key, rid)?;
            }
            deleted += 1;
        }
        Ok(deleted)
    }

    pub fn rebuild_indexes(&mut self) -> ExecutionResult<()> {
        let tuples = self.heap.scan_tuples(&self.schema)?;
        for index in &mut self.indexes {
            let text_key_size = index.index.text_key_size();
            let rebuilt = if index.key_types.len() > 1 {
                BPlusTree::create_composite(
                    self.heap.buffer_pool().clone(),
                    index.key_types.clone(),
                    Some(text_key_size),
                    index.unique,
                )?
            } else {
                BPlusTree::create(
                    self.heap.buffer_pool().clone(),
                    index.key_types[0],
                    Some(text_key_size),
                    index.unique,
                )?
            };
            for (rid, tuple) in &tuples {
                let key = Self::key_from_tuple(tuple, &index.column_indices, &index.key_types)?;
                rebuilt.insert(key, *rid)?;
            }
            index.index = rebuilt;
        }
        Ok(())
    }

    fn key_from_tuple(
        tuple: &Tuple,
        column_indices: &[usize],
        key_types: &[IndexKeyType],
    ) -> ExecutionResult<IndexKey> {
        let mut values = Vec::with_capacity(column_indices.len());
        for column_index in column_indices {
            let value = tuple
                .get(*column_index)
                .ok_or_else(|| ExecutionError::Execution("tuple missing column".to_string()))?;
            values.push(value.clone());
        }
        IndexKey::from_values(&values, key_types)
    }
}

pub struct Catalog {
    tables: HashMap<String, TableInfo>,
}

impl Catalog {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
        }
    }

    pub fn register_table(
        &mut self,
        table_name: impl Into<String>,
        schema: Schema,
        heap: TableHeap,
    ) {
        let table_name = table_name.into();
        let table = TableInfo::new(table_name, schema, heap);
        self.register_table_info(table);
    }

    pub fn register_table_info(&mut self, table: TableInfo) {
        let name = normalize_name(&table.name);
        self.tables.insert(name, table);
    }

    pub fn table(&self, table_name: &str) -> Option<&TableInfo> {
        let name = normalize_name(table_name);
        self.tables.get(&name)
    }

    pub fn table_mut(&mut self, table_name: &str) -> Option<&mut TableInfo> {
        let name = normalize_name(table_name);
        self.tables.get_mut(&name)
    }

    pub fn table_names(&self) -> Vec<String> {
        let mut names = self
            .tables
            .values()
            .map(|table| table.name.clone())
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    pub fn tables(&self) -> impl Iterator<Item = &TableInfo> {
        self.tables.values()
    }

    pub fn drop_table(&mut self, table_name: &str) -> ExecutionResult<()> {
        let name = normalize_name(table_name);
        if self.tables.remove(&name).is_some() {
            Ok(())
        } else {
            Err(ExecutionError::TableNotFound(table_name.to_string()))
        }
    }

    pub fn rename_table(&mut self, table_name: &str, new_name: &str) -> ExecutionResult<()> {
        let current_key = normalize_name(table_name);
        let next_key = normalize_name(new_name);
        if current_key == next_key {
            return Err(ExecutionError::Schema(format!(
                "table {} already has that name",
                table_name
            )));
        }
        if !self.tables.contains_key(&current_key) {
            return Err(ExecutionError::TableNotFound(table_name.to_string()));
        }
        if self.tables.contains_key(&next_key) {
            return Err(ExecutionError::Schema(format!(
                "table {} already exists",
                new_name
            )));
        }
        let mut table = self.tables.remove(&current_key).expect("table exists");
        table.rename_table(new_name);
        self.tables.insert(next_key, table);
        Ok(())
    }

    pub fn rename_column(
        &mut self,
        table_name: &str,
        old_name: &str,
        new_name: &str,
    ) -> ExecutionResult<()> {
        let table = self
            .table_mut(table_name)
            .ok_or_else(|| ExecutionError::TableNotFound(table_name.to_string()))?;
        table.rename_column(old_name, new_name)
    }

    pub fn add_column(&mut self, table_name: &str, column_def: ColumnDef) -> ExecutionResult<()> {
        let table = self
            .table_mut(table_name)
            .ok_or_else(|| ExecutionError::TableNotFound(table_name.to_string()))?;
        table.add_column(column_def)
    }

    pub fn drop_column(&mut self, table_name: &str, column_name: &str) -> ExecutionResult<()> {
        let table = self
            .table_mut(table_name)
            .ok_or_else(|| ExecutionError::TableNotFound(table_name.to_string()))?;
        table.drop_column(column_name)
    }

    pub fn insert_tuple(&self, table_name: &str, tuple: &Tuple) -> ExecutionResult<Rid> {
        let table = self
            .table(table_name)
            .ok_or_else(|| ExecutionError::TableNotFound(table_name.to_string()))?;
        table.insert_tuple(tuple)
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PhysicalPlanner<'a> {
    catalog: &'a Catalog,
}

impl<'a> PhysicalPlanner<'a> {
    pub fn new(catalog: &'a Catalog) -> Self {
        Self { catalog }
    }

    pub fn plan(&self, plan: &LogicalPlan) -> ExecutionResult<Box<dyn PhysicalOperator>> {
        Ok(self.plan_node(plan)?.operator)
    }

    fn plan_node(&self, plan: &LogicalPlan) -> ExecutionResult<PlannedOperator> {
        match plan {
            LogicalPlan::Scan {
                table_name, alias, ..
            } => {
                let table = self
                    .catalog
                    .table(table_name)
                    .ok_or_else(|| ExecutionError::TableNotFound(table_name.clone()))?;
                let schema = apply_alias(&table.schema, alias.as_deref());
                let operator = Box::new(SeqScan::new(table.heap.clone(), schema.clone()));
                Ok(PlannedOperator { operator, schema })
            }
            LogicalPlan::Filter { input, predicate } => {
                if let LogicalPlan::Scan {
                    table_name, alias, ..
                } = input.as_ref()
                {
                    if let Some(planned) =
                        self.plan_index_scan(table_name, alias.as_deref(), predicate)?
                    {
                        reject_blob_predicate(predicate, &planned.schema)?;
                        return Ok(planned);
                    }
                }
                let input_planned = self.plan_node(input)?;
                reject_blob_predicate(predicate, &input_planned.schema)?;
                let schema = input_planned.schema.clone();
                let operator = Box::new(Filter::new(
                    input_planned.operator,
                    predicate.clone(),
                    schema.clone(),
                ));
                Ok(PlannedOperator { operator, schema })
            }
            LogicalPlan::Project {
                input,
                expressions,
                aliases,
            } => {
                let input_planned = self.plan_node(input)?;
                let output_schema =
                    build_projection_schema(expressions, aliases.as_ref(), &input_planned.schema)?;
                let operator = Box::new(Projection::new(
                    input_planned.operator,
                    expressions.clone(),
                    input_planned.schema,
                ));
                Ok(PlannedOperator {
                    operator,
                    schema: output_schema,
                })
            }
            LogicalPlan::Join {
                left,
                right,
                join_type,
                condition,
            } => {
                if *join_type != JoinType::Inner {
                    return Err(ExecutionError::UnsupportedPlan(format!(
                        "only inner joins are supported, found {}",
                        join_type
                    )));
                }
                let predicate = condition.clone().ok_or_else(|| {
                    ExecutionError::UnsupportedPlan("inner join requires condition".to_string())
                })?;
                let left_planned = self.plan_node(left)?;
                let right_planned = self.plan_node(right)?;
                let mut fields = left_planned.schema.fields.clone();
                fields.extend(right_planned.schema.fields.clone());
                let output_schema = Schema::new(fields);
                reject_blob_predicate(&predicate, &output_schema)?;
                let operator = Box::new(NestedLoopJoin::new(
                    left_planned.operator,
                    right_planned.operator,
                    predicate,
                    left_planned.schema,
                    right_planned.schema,
                ));
                Ok(PlannedOperator {
                    operator,
                    schema: output_schema,
                })
            }
            LogicalPlan::Update {
                table_name,
                assignments,
                filter,
                ..
            } => {
                let table = self
                    .catalog
                    .table(table_name)
                    .ok_or_else(|| ExecutionError::TableNotFound(table_name.clone()))?;
                let operator = Box::new(Update::new(
                    table.clone(),
                    assignments.clone(),
                    filter.clone(),
                ));
                Ok(PlannedOperator {
                    operator,
                    schema: table.schema.clone(),
                })
            }
            _ => Err(ExecutionError::UnsupportedPlan(format!(
                "logical plan {:?} is not supported in execution",
                plan
            ))),
        }
    }

    fn plan_index_scan(
        &self,
        table_name: &str,
        alias: Option<&str>,
        predicate: &Expr,
    ) -> ExecutionResult<Option<PlannedOperator>> {
        let table = self
            .catalog
            .table(table_name)
            .ok_or_else(|| ExecutionError::TableNotFound(table_name.to_string()))?;
        let (index, index_predicate) = match extract_index_predicate(predicate, table, alias)? {
            Some(info) => info,
            None => return Ok(None),
        };
        let schema = apply_alias(&table.schema, alias);
        let operator = Box::new(IndexScan::new(
            table.heap.clone(),
            schema.clone(),
            index.index.clone(),
            index_predicate,
        ));
        Ok(Some(PlannedOperator { operator, schema }))
    }
}

struct PlannedOperator {
    operator: Box<dyn PhysicalOperator>,
    schema: Schema,
}

fn normalize_name(name: &str) -> String {
    name.to_lowercase()
}

fn apply_alias(schema: &Schema, alias: Option<&str>) -> Schema {
    if let Some(alias_name) = alias {
        Schema::new(
            schema
                .fields
                .iter()
                .map(|field| Field {
                    name: format!("{}.{}", alias_name, field.name),
                    table: Some(alias_name.to_string()),
                    data_type: field.data_type.clone(),
                    nullable: field.nullable,
                    visible: field.visible,
                })
                .collect(),
        )
    } else {
        schema.clone()
    }
}

fn build_projection_schema(
    expressions: &[Expr],
    aliases: Option<&Vec<String>>,
    input_schema: &Schema,
) -> ExecutionResult<Schema> {
    let mut fields = Vec::new();
    for (index, expr) in expressions.iter().enumerate() {
        match expr {
            Expr::Wildcard => {
                fields.extend(
                    input_schema
                        .fields
                        .iter()
                        .filter(|field| field.visible)
                        .cloned(),
                );
            }
            Expr::QualifiedWildcard { table } => {
                let mut matched = false;
                for field in &input_schema.fields {
                    if !field.visible {
                        continue;
                    }
                    let table_matches = field
                        .table
                        .as_ref()
                        .map(|field_table| field_table.eq_ignore_ascii_case(table))
                        .unwrap_or(false)
                        || field
                            .name
                            .split('.')
                            .next()
                            .map(|prefix| prefix.eq_ignore_ascii_case(table))
                            .unwrap_or(false);
                    if table_matches {
                        matched = true;
                        fields.push(field.clone());
                    }
                }
                if !matched {
                    return Err(ExecutionError::Schema(format!(
                        "qualified wildcard {} did not match any columns",
                        table
                    )));
                }
            }
            Expr::Literal(crate::expr::LiteralValue::Blob(_)) => {
                let name = aliases
                    .and_then(|aliases| aliases.get(index).cloned())
                    .unwrap_or_else(|| expr.to_string());
                fields.push(Field {
                    name,
                    table: None,
                    data_type: DataType::Blob,
                    nullable: true,
                    visible: true,
                });
            }
            _ => {
                let name = aliases
                    .and_then(|aliases| aliases.get(index).cloned())
                    .unwrap_or_else(|| expr.to_string());
                fields.push(Field {
                    name,
                    table: None,
                    data_type: DataType::Text,
                    nullable: true,
                    visible: true,
                });
            }
        }
    }
    Ok(Schema::new(fields))
}

fn index_key_type_for_data_type(data_type: &DataType) -> ExecutionResult<IndexKeyType> {
    match data_type {
        DataType::Integer | DataType::BigInt | DataType::Timestamp => Ok(IndexKeyType::Integer),
        DataType::Text => Ok(IndexKeyType::Text),
        DataType::Blob => Err(ExecutionError::Execution(
            "BLOB columns cannot be indexed".to_string(),
        )),
        other => Err(ExecutionError::Execution(format!(
            "unsupported index key type {:?}",
            other
        ))),
    }
}

fn extract_index_predicate(
    predicate: &Expr,
    table: &TableInfo,
    alias: Option<&str>,
) -> ExecutionResult<Option<(IndexInfo, IndexPredicate)>> {
    match predicate {
        Expr::BinaryOp { left, op, right } => {
            let (column_table, column_name, literal, op) = match (left.as_ref(), right.as_ref()) {
                (Expr::Column { table, name }, Expr::Literal(literal)) => {
                    (table.as_deref(), name.as_str(), literal, *op)
                }
                (Expr::Literal(literal), Expr::Column { table, name }) => {
                    let flipped = match flip_comparison_operator(*op) {
                        Some(op) => op,
                        None => return Ok(None),
                    };
                    (table.as_deref(), name.as_str(), literal, flipped)
                }
                _ => return Ok(None),
            };
            if !column_matches(column_table, &table.name, alias) {
                return Ok(None);
            }
            let index = match table.index_for_column(column_name) {
                Some(index) => index.clone(),
                None => return Ok(None),
            };
            let value = Value::from(literal);
            let key_type = *index
                .key_types
                .first()
                .ok_or_else(|| ExecutionError::Execution("index key types missing".to_string()))?;
            let key = match IndexKey::from_value(&value, key_type) {
                Ok(key) => key,
                Err(_) => return Ok(None),
            };
            let predicate = match op {
                BinaryOperator::Eq => IndexPredicate::equality(key),
                BinaryOperator::Lt => IndexPredicate {
                    lower: None,
                    upper: Some((key, false)),
                },
                BinaryOperator::LtEq => IndexPredicate {
                    lower: None,
                    upper: Some((key, true)),
                },
                BinaryOperator::Gt => IndexPredicate {
                    lower: Some((key, false)),
                    upper: None,
                },
                BinaryOperator::GtEq => IndexPredicate {
                    lower: Some((key, true)),
                    upper: None,
                },
                _ => return Ok(None),
            };
            Ok(Some((index, predicate)))
        }
        Expr::Between {
            expr,
            low,
            high,
            negated,
        } => {
            if *negated {
                return Ok(None);
            }
            let (column_table, column_name) = match expr.as_ref() {
                Expr::Column { table, name } => (table.as_deref(), name.as_str()),
                _ => return Ok(None),
            };
            if !column_matches(column_table, &table.name, alias) {
                return Ok(None);
            }
            let index = match table.index_for_column(column_name) {
                Some(index) => index.clone(),
                None => return Ok(None),
            };
            let low_literal = match low.as_ref() {
                Expr::Literal(literal) => literal,
                _ => return Ok(None),
            };
            let high_literal = match high.as_ref() {
                Expr::Literal(literal) => literal,
                _ => return Ok(None),
            };
            let low_value = Value::from(low_literal);
            let high_value = Value::from(high_literal);
            let key_type = *index
                .key_types
                .first()
                .ok_or_else(|| ExecutionError::Execution("index key types missing".to_string()))?;
            let low_key = match IndexKey::from_value(&low_value, key_type) {
                Ok(key) => key,
                Err(_) => return Ok(None),
            };
            let high_key = match IndexKey::from_value(&high_value, key_type) {
                Ok(key) => key,
                Err(_) => return Ok(None),
            };
            Ok(Some((
                index,
                IndexPredicate {
                    lower: Some((low_key, true)),
                    upper: Some((high_key, true)),
                },
            )))
        }
        _ => Ok(None),
    }
}

fn column_matches(column_table: Option<&str>, table_name: &str, alias: Option<&str>) -> bool {
    match column_table {
        None => true,
        Some(name) => {
            name.eq_ignore_ascii_case(table_name)
                || alias
                    .map(|alias| name.eq_ignore_ascii_case(alias))
                    .unwrap_or(false)
        }
    }
}

fn reject_blob_predicate(expr: &Expr, schema: &Schema) -> ExecutionResult<()> {
    if expr_uses_blob(expr, schema)? {
        return Err(ExecutionError::UnsupportedExpression(
            "BLOB columns do not support predicate expressions".to_string(),
        ));
    }
    Ok(())
}

fn expr_uses_blob(expr: &Expr, schema: &Schema) -> ExecutionResult<bool> {
    match expr {
        Expr::Column { table, name } => Ok(field_is_blob(schema, table.as_deref(), name)),
        Expr::Literal(literal) => Ok(matches!(literal, crate::expr::LiteralValue::Blob(_))),
        Expr::BinaryOp { left, right, .. } => {
            Ok(expr_uses_blob(left, schema)? || expr_uses_blob(right, schema)?)
        }
        Expr::UnaryOp { expr, .. } => expr_uses_blob(expr, schema),
        Expr::Function { args, .. } => {
            for arg in args {
                if expr_uses_blob(arg, schema)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Expr::Cast { expr, .. } => expr_uses_blob(expr, schema),
        Expr::IsNull { expr, .. } => expr_uses_blob(expr, schema),
        Expr::Between {
            expr, low, high, ..
        } => Ok(expr_uses_blob(expr, schema)?
            || expr_uses_blob(low, schema)?
            || expr_uses_blob(high, schema)?),
        Expr::In { expr, list, .. } => {
            if expr_uses_blob(expr, schema)? {
                return Ok(true);
            }
            for item in list {
                if expr_uses_blob(item, schema)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Expr::Wildcard | Expr::QualifiedWildcard { .. } => Ok(false),
    }
}

fn field_is_blob(schema: &Schema, table: Option<&str>, name: &str) -> bool {
    let qualified = table.map(|table| format!("{}.{}", table, name));
    schema.fields.iter().any(|field| {
        let name_matches = field.name.eq_ignore_ascii_case(name)
            || qualified
                .as_ref()
                .map(|qualified| field.name.eq_ignore_ascii_case(qualified))
                .unwrap_or(false)
            || field
                .name
                .split('.')
                .next_back()
                .map(|segment| segment.eq_ignore_ascii_case(name))
                .unwrap_or(false);
        let table_matches = match (table, field.table.as_deref()) {
            (Some(table_name), Some(field_table)) => field_table.eq_ignore_ascii_case(table_name),
            (None, _) => true,
            _ => false,
        };
        name_matches && table_matches && field.data_type == DataType::Blob
    })
}

fn flip_comparison_operator(op: BinaryOperator) -> Option<BinaryOperator> {
    match op {
        BinaryOperator::Eq => Some(BinaryOperator::Eq),
        BinaryOperator::Lt => Some(BinaryOperator::Gt),
        BinaryOperator::LtEq => Some(BinaryOperator::GtEq),
        BinaryOperator::Gt => Some(BinaryOperator::Lt),
        BinaryOperator::GtEq => Some(BinaryOperator::LtEq),
        _ => None,
    }
}

fn apply_assignments(
    tuple: &Tuple,
    schema: &Schema,
    assignments: &[Assignment],
) -> ExecutionResult<Tuple> {
    let mut values = tuple.values().to_vec();
    for assignment in assignments {
        let index = schema.field_index(&assignment.column).ok_or_else(|| {
            ExecutionError::Schema(format!("column {} not found", assignment.column))
        })?;
        if index >= values.len() {
            return Err(ExecutionError::Schema(format!(
                "column index {} out of range",
                index
            )));
        }
        values[index] = evaluate_expr(&assignment.value, tuple, schema)?;
    }
    Ok(Tuple::new(values))
}
