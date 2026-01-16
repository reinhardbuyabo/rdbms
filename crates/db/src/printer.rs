use comfy_table::{Cell, Table};
use serde::{Deserialize, Serialize};
use std::fmt;

use query::{Schema, Tuple, Value};

const MAX_DISPLAY_ROWS: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub enum ReplOutput {
    Rows { schema: Schema, rows: Vec<Tuple> },
    Message(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableRow {
    pub values: Vec<SerializableValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SerializableValue {
    Null,
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(String),
    Blob(Vec<u8>),
}

impl From<Value> for SerializableValue {
    fn from(v: Value) -> Self {
        match v {
            Value::Null => SerializableValue::Null,
            Value::Integer(n) => SerializableValue::Int(n),
            Value::Float(f) => SerializableValue::Float(f),
            Value::Boolean(b) => SerializableValue::Bool(b),
            Value::String(s) => SerializableValue::Text(s),
            Value::Blob(bytes) => SerializableValue::Blob(bytes),
            Value::Timestamp(ts) => SerializableValue::Int(ts),
        }
    }
}

impl fmt::Display for ReplOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplOutput::Rows { schema, rows } => write!(f, "{}", format_table(schema, rows)),
            ReplOutput::Message(message) => write!(f, "{}", message),
        }
    }
}

pub fn format_output(output: &ReplOutput) -> String {
    match output {
        ReplOutput::Rows { schema, rows } => format_table(schema, rows),
        ReplOutput::Message(message) => message.to_string(),
    }
}

pub fn print_output(output: &ReplOutput) {
    println!("{}", format_output(output));
}

fn format_table(schema: &Schema, rows: &[Tuple]) -> String {
    let total_rows = rows.len();
    let mut table = Table::new();
    let headers = schema
        .fields
        .iter()
        .map(|field| Cell::new(field.name.clone()))
        .collect::<Vec<_>>();
    table.set_header(headers);

    for row in rows.iter().take(MAX_DISPLAY_ROWS) {
        let cells = row
            .values()
            .iter()
            .map(|value| Cell::new(format_value(value)))
            .collect::<Vec<_>>();
        table.add_row(cells);
    }

    let mut output = table.to_string();
    output.push('\n');
    output.push_str(&format!("({} rows)", total_rows));

    let hidden_rows = total_rows.saturating_sub(MAX_DISPLAY_ROWS);
    if hidden_rows > 0 {
        output.push('\n');
        output.push_str(&format!("... ({} rows hidden)", hidden_rows));
    }

    output
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Integer(number) => number.to_string(),
        Value::Float(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Boolean(flag) => flag.to_string(),
        Value::Timestamp(number) => number.to_string(),
        Value::Blob(bytes) => format_blob_preview(bytes),
    }
}

fn format_blob_preview(bytes: &[u8]) -> String {
    let preview_len = bytes.len().min(16);
    let preview = bytes[..preview_len]
        .iter()
        .map(|byte| format!("{:02X}", byte))
        .collect::<String>();
    let suffix = if bytes.len() > preview_len { "…" } else { "" };
    let size_label = format_blob_size(bytes.len());
    if preview.is_empty() {
        format!("<BLOB size={}>", size_label)
    } else {
        format!("<BLOB 0x{}{} size={}>", preview, suffix, size_label)
    }
}

fn format_blob_size(len: usize) -> String {
    if len >= 1024 * 1024 {
        format!("{}MB", len.div_ceil(1024 * 1024))
    } else if len >= 1024 {
        format!("{}KB", len.div_ceil(1024))
    } else {
        format!("{}B", len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use query::{DataType, Field};

    fn sample_schema() -> Schema {
        Schema::new(vec![Field {
            name: "value".to_string(),
            table: None,
            data_type: DataType::Text,
            nullable: true,
            visible: true,
        }])
    }

    fn row_with_value(value: Value) -> Tuple {
        Tuple::new(vec![value])
    }

    #[test]
    fn formats_table_output() {
        let schema = Schema::new(vec![
            Field {
                name: "id".to_string(),
                table: Some("users".to_string()),
                data_type: DataType::Integer,
                nullable: false,
                visible: true,
            },
            Field {
                name: "name".to_string(),
                table: Some("users".to_string()),
                data_type: DataType::Text,
                nullable: false,
                visible: true,
            },
        ]);
        let rows = vec![Tuple::new(vec![
            Value::Integer(1),
            Value::String("Ada".to_string()),
        ])];
        let output = format_output(&ReplOutput::Rows { schema, rows });
        assert!(output.contains("id"));
        assert!(output.contains("Ada"));
        assert!(output.contains("(1 rows)"));
    }

    #[test]
    fn formats_empty_result_set() {
        let schema = sample_schema();
        let rows = Vec::new();
        let output = format_output(&ReplOutput::Rows { schema, rows });
        assert!(output.contains("(0 rows)"));
        assert!(!output.contains("rows hidden"));
    }

    #[test]
    fn formats_null_values() {
        let schema = sample_schema();
        let rows = vec![row_with_value(Value::Null)];
        let output = format_output(&ReplOutput::Rows { schema, rows });
        assert!(output.contains("NULL"));
        assert!(output.contains("(1 rows)"));
    }

    #[test]
    fn formats_blob_values() {
        let schema = Schema::new(vec![Field {
            name: "payload".to_string(),
            table: None,
            data_type: DataType::Blob,
            nullable: false,
            visible: true,
        }]);
        let blob = Value::Blob(vec![0x89, 0x50, 0x4E, 0x47]);
        let rows = vec![Tuple::new(vec![blob])];
        let output = format_output(&ReplOutput::Rows { schema, rows });
        assert!(output.contains("<BLOB"));
        assert!(output.contains("size=4B"));
        assert!(!output.contains("PNG"));
    }

    #[test]
    fn respects_display_limit_boundary() {
        let schema = sample_schema();
        let rows = (0..MAX_DISPLAY_ROWS)
            .map(|idx| row_with_value(Value::String(format!("row{}", idx))))
            .collect::<Vec<_>>();
        let output = format_output(&ReplOutput::Rows { schema, rows });
        assert!(output.contains(&format!("({} rows)", MAX_DISPLAY_ROWS)));
        assert!(!output.contains("rows hidden"));
    }

    #[test]
    fn truncates_overflow_rows() {
        let schema = sample_schema();
        let mut rows = (0..MAX_DISPLAY_ROWS)
            .map(|idx| row_with_value(Value::String(format!("row{}", idx))))
            .collect::<Vec<_>>();
        rows.push(row_with_value(Value::String("hidden_row".to_string())));
        rows.push(row_with_value(Value::String("hidden_row_2".to_string())));
        let output = format_output(&ReplOutput::Rows { schema, rows });
        assert!(output.contains(&format!("({} rows)", MAX_DISPLAY_ROWS + 2)));
        assert!(output.contains("... (2 rows hidden)"));
        assert!(!output.contains("hidden_row"));
    }

    #[test]
    fn handles_large_result_sets() {
        let schema = Schema::new(vec![Field {
            name: "value".to_string(),
            table: None,
            data_type: DataType::Integer,
            nullable: false,
            visible: true,
        }]);
        let rows = (0..100_000)
            .map(|idx| Tuple::new(vec![Value::Integer(idx as i64)]))
            .collect::<Vec<_>>();
        let output = format_output(&ReplOutput::Rows { schema, rows });
        assert!(output.contains("(100000 rows)"));
        assert!(output.contains("... (99900 rows hidden)"));
    }

    #[test]
    fn handles_long_string_values() {
        let schema = sample_schema();
        let long_text = "x".repeat(256);
        let rows = vec![row_with_value(Value::String(long_text.clone()))];
        let output = format_output(&ReplOutput::Rows { schema, rows });
        assert!(output.contains(&long_text));
    }
}
