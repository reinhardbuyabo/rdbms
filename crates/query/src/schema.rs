use serde::{Deserialize, Serialize};

/// Represents a SQL data type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    Integer,
    BigInt,
    Real,
    Text,
    Boolean,
    Timestamp,
    Blob,
}

impl DataType {
    pub fn fixed_size(&self) -> Option<usize> {
        match self {
            DataType::Integer => Some(4),
            DataType::BigInt => Some(8),
            DataType::Real => Some(8),
            DataType::Boolean => Some(1),
            DataType::Timestamp => Some(8),
            DataType::Text | DataType::Blob => None,
        }
    }
    pub fn is_nullable_by_default(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub primary_key: bool,
    pub unique: bool,
    pub default_value: Option<DefaultValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DefaultValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Boolean(bool),
    CurrentTimestamp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableSchema {
    pub table_name: String,
    pub columns: Vec<ColumnDef>,
}

impl TableSchema {
    pub fn new(table_name: String, columns: Vec<ColumnDef>) -> Self {
        Self {
            table_name,
            columns,
        }
    }
    pub fn find_column(&self, name: &str) -> Option<&ColumnDef> {
        self.columns
            .iter()
            .find(|col| col.name.eq_ignore_ascii_case(name))
    }
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|col| col.name.eq_ignore_ascii_case(name))
    }
    pub fn column_names(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.name.clone()).collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub table: Option<String>,
    pub data_type: DataType,
    pub nullable: bool,
    pub visible: bool,
}

impl Schema {
    pub fn empty() -> Self {
        Self { fields: Vec::new() }
    }
    pub fn new(fields: Vec<Field>) -> Self {
        Self { fields }
    }
    pub fn visible_schema(&self) -> Self {
        Self::new(self.fields.iter().filter(|f| f.visible).cloned().collect())
    }
    pub fn visible_fields(&self) -> impl Iterator<Item = &Field> {
        self.fields.iter().filter(|field| field.visible)
    }
    pub fn find_field(&self, name: &str) -> Option<&Field> {
        if let Some(field) = self
            .fields
            .iter()
            .find(|f| f.visible && f.name.eq_ignore_ascii_case(name))
        {
            return Some(field);
        }
        if let Some(field) = self.fields.iter().find(|f| {
            f.visible
                && f.name
                    .split('.')
                    .next_back()
                    .unwrap_or("")
                    .eq_ignore_ascii_case(name)
        }) {
            return Some(field);
        }
        None
    }
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields
            .iter()
            .position(|f| f.visible && f.name.eq_ignore_ascii_case(name))
    }
}
