use crate::execution::tuple::{Tuple, Value};
use crate::expr::{BinaryOperator, Expr, UnaryOperator};
use crate::schema::{DataType, Schema};
use std::cmp::Ordering;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("storage error: {0}")]
    Storage(#[from] storage::BufferPoolError),
    #[error("table not found: {0}")]
    TableNotFound(String),
    #[error("expression error: {0}")]
    Expression(String),
    #[error("schema error: {0}")]
    Schema(String),
    #[error("unsupported plan: {0}")]
    UnsupportedPlan(String),
    #[error("unsupported expression: {0}")]
    UnsupportedExpression(String),
    #[error("execution error: {0}")]
    Execution(String),
}

pub type ExecutionResult<T> = Result<T, ExecutionError>;

pub trait PhysicalOperator {
    fn open(&mut self) -> ExecutionResult<()>;
    fn next(&mut self) -> ExecutionResult<Option<Tuple>>;
    fn close(&mut self) -> ExecutionResult<()>;
}

pub fn evaluate_predicate(expr: &Expr, tuple: &Tuple, schema: &Schema) -> ExecutionResult<bool> {
    let value = evaluate_expr(expr, tuple, schema)?;
    match value {
        Value::Boolean(flag) => Ok(flag),
        Value::Null => Ok(false),
        other => Err(ExecutionError::Expression(format!(
            "predicate returned non-boolean value: {:?}",
            other
        ))),
    }
}

pub fn evaluate_expr(expr: &Expr, tuple: &Tuple, schema: &Schema) -> ExecutionResult<Value> {
    match expr {
        Expr::Column { table, name } => {
            let index = resolve_column_index(schema, table.as_deref(), name)?;
            tuple.get(index).cloned().ok_or_else(|| {
                ExecutionError::Schema(format!("column index {} out of range", index))
            })
        }
        Expr::Literal(literal) => Ok(Value::from(literal)),
        Expr::BinaryOp { left, op, right } => {
            let left_value = evaluate_expr(left, tuple, schema)?;
            let right_value = evaluate_expr(right, tuple, schema)?;
            apply_binary_operator(*op, left_value, right_value)
        }
        Expr::UnaryOp { op, expr } => {
            let value = evaluate_expr(expr, tuple, schema)?;
            apply_unary_operator(*op, value)
        }
        Expr::Function { name, .. } => Err(ExecutionError::UnsupportedExpression(format!(
            "function {} is not supported",
            name
        ))),
        Expr::Wildcard => Err(ExecutionError::UnsupportedExpression(
            "wildcard expression must be expanded in projection".to_string(),
        )),
        Expr::QualifiedWildcard { table } => Err(ExecutionError::UnsupportedExpression(format!(
            "qualified wildcard {} must be expanded in projection",
            table
        ))),
        Expr::Cast { expr, target_type } => {
            let value = evaluate_expr(expr, tuple, schema)?;
            apply_cast(value, target_type)
        }
        Expr::IsNull { expr, negated } => {
            let value = evaluate_expr(expr, tuple, schema)?;
            let is_null = value.is_null();
            Ok(Value::Boolean(if *negated { !is_null } else { is_null }))
        }
        Expr::Between {
            expr,
            low,
            high,
            negated,
        } => {
            let value = evaluate_expr(expr, tuple, schema)?;
            let lower = evaluate_expr(low, tuple, schema)?;
            let upper = evaluate_expr(high, tuple, schema)?;
            let lower_check = apply_comparison(BinaryOperator::GtEq, &value, &lower)?;
            let upper_check = apply_comparison(BinaryOperator::LtEq, &value, &upper)?;
            let combined = apply_binary_operator(BinaryOperator::And, lower_check, upper_check)?;
            let result = match combined {
                Value::Boolean(flag) => Value::Boolean(if *negated { !flag } else { flag }),
                Value::Null => Value::Null,
                other => {
                    return Err(ExecutionError::Expression(format!(
                        "between expression produced non-boolean value: {:?}",
                        other
                    )));
                }
            };
            Ok(result)
        }
        Expr::In {
            expr,
            list,
            negated,
        } => {
            let value = evaluate_expr(expr, tuple, schema)?;
            let mut saw_null = false;
            for item in list {
                let item_value = evaluate_expr(item, tuple, schema)?;
                let comparison = apply_comparison(BinaryOperator::Eq, &value, &item_value)?;
                match comparison {
                    Value::Boolean(true) => {
                        return Ok(Value::Boolean(!*negated));
                    }
                    Value::Null => saw_null = true,
                    _ => {}
                }
            }
            if saw_null {
                Ok(Value::Null)
            } else {
                Ok(Value::Boolean(*negated))
            }
        }
    }
}

fn resolve_column_index(
    schema: &Schema,
    table: Option<&str>,
    name: &str,
) -> ExecutionResult<usize> {
    let qualified_name = table.map(|table_name| format!("{}.{}", table_name, name));
    let mut matches = Vec::new();

    for (index, field) in schema.fields.iter().enumerate() {
        let base_matches = field.name.eq_ignore_ascii_case(name)
            || field
                .name
                .split('.')
                .next_back()
                .map(|segment| segment.eq_ignore_ascii_case(name))
                .unwrap_or(false);
        let qualified_matches = qualified_name
            .as_ref()
            .map(|qualified| field.name.eq_ignore_ascii_case(qualified))
            .unwrap_or(false);
        let table_matches = match (table, field.table.as_ref()) {
            (Some(table_name), Some(field_table)) => field_table.eq_ignore_ascii_case(table_name),
            (None, _) => true,
            _ => false,
        };
        if (base_matches || qualified_matches) && table_matches {
            matches.push(index);
        }
    }

    match matches.len() {
        0 => Err(ExecutionError::Schema(format!(
            "column {} not found",
            qualified_name.as_deref().unwrap_or(name)
        ))),
        1 => Ok(matches[0]),
        _ => Err(ExecutionError::Schema(format!(
            "column reference {} is ambiguous",
            qualified_name.as_deref().unwrap_or(name)
        ))),
    }
}

fn apply_binary_operator(op: BinaryOperator, left: Value, right: Value) -> ExecutionResult<Value> {
    match op {
        BinaryOperator::Plus
        | BinaryOperator::Minus
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Modulo => apply_numeric_operator(op, &left, &right),
        BinaryOperator::Eq
        | BinaryOperator::NotEq
        | BinaryOperator::Lt
        | BinaryOperator::LtEq
        | BinaryOperator::Gt
        | BinaryOperator::GtEq => apply_comparison(op, &left, &right),
        BinaryOperator::And => apply_boolean_logic(op, &left, &right),
        BinaryOperator::Or => apply_boolean_logic(op, &left, &right),
        BinaryOperator::Concat => apply_concat(&left, &right),
        BinaryOperator::Like | BinaryOperator::NotLike => apply_like(op, &left, &right),
    }
}

fn apply_unary_operator(op: UnaryOperator, value: Value) -> ExecutionResult<Value> {
    match op {
        UnaryOperator::Not => match boolean_from_value(&value)? {
            Some(flag) => Ok(Value::Boolean(!flag)),
            None => Ok(Value::Null),
        },
        UnaryOperator::Minus => match numeric_from_value(&value)? {
            Some(NumericValue::Integer(number)) => Ok(Value::Integer(-number)),
            Some(NumericValue::Float(number)) => Ok(Value::Float(-number)),
            None => Ok(Value::Null),
        },
        UnaryOperator::Plus => match numeric_from_value(&value)? {
            Some(NumericValue::Integer(number)) => Ok(Value::Integer(number)),
            Some(NumericValue::Float(number)) => Ok(Value::Float(number)),
            None => Ok(Value::Null),
        },
    }
}

fn apply_numeric_operator(
    op: BinaryOperator,
    left: &Value,
    right: &Value,
) -> ExecutionResult<Value> {
    let (left_value, right_value, both_integer) = match numeric_pair(left, right)? {
        Some(values) => values,
        None => return Ok(Value::Null),
    };

    let result = match op {
        BinaryOperator::Plus => numeric_result(left_value + right_value, both_integer),
        BinaryOperator::Minus => numeric_result(left_value - right_value, both_integer),
        BinaryOperator::Multiply => numeric_result(left_value * right_value, both_integer),
        BinaryOperator::Divide => {
            if right_value == 0.0 {
                return Err(ExecutionError::Expression("division by zero".to_string()));
            }
            Value::Float(left_value / right_value)
        }
        BinaryOperator::Modulo => {
            if !both_integer {
                return Err(ExecutionError::Expression(
                    "modulo requires integer operands".to_string(),
                ));
            }
            let left_int = left_value as i64;
            let right_int = right_value as i64;
            if right_int == 0 {
                return Err(ExecutionError::Expression("modulo by zero".to_string()));
            }
            Value::Integer(left_int % right_int)
        }
        _ => {
            return Err(ExecutionError::Expression(
                "invalid numeric operator".to_string(),
            ));
        }
    };

    Ok(result)
}

fn numeric_result(value: f64, as_integer: bool) -> Value {
    if as_integer {
        Value::Integer(value as i64)
    } else {
        Value::Float(value)
    }
}

fn apply_comparison(op: BinaryOperator, left: &Value, right: &Value) -> ExecutionResult<Value> {
    if left.is_null() || right.is_null() {
        return Ok(Value::Null);
    }

    let ordering = compare_values(left, right)?;
    let ordering = match ordering {
        Some(value) => value,
        None => return Ok(Value::Null),
    };

    let result = match op {
        BinaryOperator::Eq => ordering == Ordering::Equal,
        BinaryOperator::NotEq => ordering != Ordering::Equal,
        BinaryOperator::Lt => ordering == Ordering::Less,
        BinaryOperator::LtEq => ordering != Ordering::Greater,
        BinaryOperator::Gt => ordering == Ordering::Greater,
        BinaryOperator::GtEq => ordering != Ordering::Less,
        _ => {
            return Err(ExecutionError::Expression(
                "invalid comparison operator".to_string(),
            ));
        }
    };

    Ok(Value::Boolean(result))
}

fn apply_boolean_logic(op: BinaryOperator, left: &Value, right: &Value) -> ExecutionResult<Value> {
    let left_value = boolean_from_value(left)?;
    let right_value = boolean_from_value(right)?;
    let result = match op {
        BinaryOperator::And => tri_and(left_value, right_value),
        BinaryOperator::Or => tri_or(left_value, right_value),
        _ => None,
    };
    Ok(result.map_or(Value::Null, Value::Boolean))
}

fn tri_and(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(false), _) | (_, Some(false)) => Some(false),
        (Some(true), Some(true)) => Some(true),
        (Some(true), None) | (None, Some(true)) | (None, None) => None,
    }
}

fn tri_or(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), Some(false)) => Some(false),
        (Some(false), None) | (None, Some(false)) | (None, None) => None,
    }
}

fn apply_concat(left: &Value, right: &Value) -> ExecutionResult<Value> {
    if left.is_null() || right.is_null() {
        return Ok(Value::Null);
    }
    let left_string = value_to_string(left)?;
    let right_string = value_to_string(right)?;
    Ok(Value::String(format!("{}{}", left_string, right_string)))
}

fn apply_like(op: BinaryOperator, left: &Value, right: &Value) -> ExecutionResult<Value> {
    if left.is_null() || right.is_null() {
        return Ok(Value::Null);
    }
    let left_string = value_to_string(left)?;
    let right_string = value_to_string(right)?;
    let matches = like_match(&left_string, &right_string);
    let result = if matches {
        Value::Boolean(true)
    } else {
        Value::Boolean(false)
    };
    if matches {
        Ok(match op {
            BinaryOperator::Like => Value::Boolean(true),
            BinaryOperator::NotLike => Value::Boolean(false),
            _ => result,
        })
    } else {
        Ok(match op {
            BinaryOperator::Like => Value::Boolean(false),
            BinaryOperator::NotLike => Value::Boolean(true),
            _ => result,
        })
    }
}

fn like_match(value: &str, pattern: &str) -> bool {
    let value_chars: Vec<char> = value.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let value_len = value_chars.len();
    let pattern_len = pattern_chars.len();
    let mut dp = vec![vec![false; pattern_len + 1]; value_len + 1];
    dp[0][0] = true;

    for pattern_index in 1..=pattern_len {
        if pattern_chars[pattern_index - 1] == '%' {
            dp[0][pattern_index] = dp[0][pattern_index - 1];
        }
    }

    for value_index in 1..=value_len {
        for pattern_index in 1..=pattern_len {
            let pattern_char = pattern_chars[pattern_index - 1];
            dp[value_index][pattern_index] = match pattern_char {
                '%' => dp[value_index][pattern_index - 1] || dp[value_index - 1][pattern_index],
                '_' => dp[value_index - 1][pattern_index - 1],
                _ => {
                    dp[value_index - 1][pattern_index - 1]
                        && value_chars[value_index - 1] == pattern_char
                }
            };
        }
    }

    dp[value_len][pattern_len]
}

fn compare_values(left: &Value, right: &Value) -> ExecutionResult<Option<Ordering>> {
    if left.is_null() || right.is_null() {
        return Ok(None);
    }
    match (left, right) {
        (Value::String(left_value), Value::String(right_value)) => {
            Ok(Some(left_value.cmp(right_value)))
        }
        (Value::Boolean(left_value), Value::Boolean(right_value)) => {
            Ok(Some(left_value.cmp(right_value)))
        }
        _ => {
            let (left_value, right_value, _) = match numeric_pair(left, right)? {
                Some(values) => values,
                None => return Ok(None),
            };
            let ordering = left_value
                .partial_cmp(&right_value)
                .ok_or_else(|| ExecutionError::Expression("comparison failed".to_string()))?;
            Ok(Some(ordering))
        }
    }
}

fn boolean_from_value(value: &Value) -> ExecutionResult<Option<bool>> {
    match value {
        Value::Boolean(flag) => Ok(Some(*flag)),
        Value::Null => Ok(None),
        other => Err(ExecutionError::Expression(format!(
            "expected boolean value, found {:?}",
            other
        ))),
    }
}

#[derive(Debug, Clone, Copy)]
enum NumericValue {
    Integer(i64),
    Float(f64),
}

fn numeric_from_value(value: &Value) -> ExecutionResult<Option<NumericValue>> {
    match value {
        Value::Null => Ok(None),
        Value::Integer(number) => Ok(Some(NumericValue::Integer(*number))),
        Value::Timestamp(number) => Ok(Some(NumericValue::Integer(*number))),
        Value::Float(number) => Ok(Some(NumericValue::Float(*number))),
        other => Err(ExecutionError::Expression(format!(
            "expected numeric value, found {:?}",
            other
        ))),
    }
}

fn numeric_pair(left: &Value, right: &Value) -> ExecutionResult<Option<(f64, f64, bool)>> {
    let left_value = numeric_from_value(left)?;
    let right_value = numeric_from_value(right)?;
    match (left_value, right_value) {
        (Some(NumericValue::Integer(left_number)), Some(NumericValue::Integer(right_number))) => {
            Ok(Some((left_number as f64, right_number as f64, true)))
        }
        (Some(NumericValue::Integer(left_number)), Some(NumericValue::Float(right_number))) => {
            Ok(Some((left_number as f64, right_number, false)))
        }
        (Some(NumericValue::Float(left_number)), Some(NumericValue::Integer(right_number))) => {
            Ok(Some((left_number, right_number as f64, false)))
        }
        (Some(NumericValue::Float(left_number)), Some(NumericValue::Float(right_number))) => {
            Ok(Some((left_number, right_number, false)))
        }
        (None, _) | (_, None) => Ok(None),
    }
}

fn apply_cast(value: Value, target_type: &DataType) -> ExecutionResult<Value> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    match target_type {
        DataType::Integer | DataType::BigInt => match value {
            Value::Integer(number) => Ok(Value::Integer(number)),
            Value::Timestamp(number) => Ok(Value::Integer(number)),
            Value::Float(number) => Ok(Value::Integer(number as i64)),
            Value::Boolean(flag) => Ok(Value::Integer(i64::from(flag))),
            Value::String(text) => text.parse::<i64>().map(Value::Integer).map_err(|_| {
                ExecutionError::Expression(format!("cannot cast '{}' to integer", text))
            }),
            other => Err(ExecutionError::Expression(format!(
                "cannot cast {:?} to integer",
                other
            ))),
        },
        DataType::Real => match value {
            Value::Integer(number) => Ok(Value::Float(number as f64)),
            Value::Timestamp(number) => Ok(Value::Float(number as f64)),
            Value::Float(number) => Ok(Value::Float(number)),
            Value::Boolean(flag) => Ok(Value::Float(if flag { 1.0 } else { 0.0 })),
            Value::String(text) => text
                .parse::<f64>()
                .map(Value::Float)
                .map_err(|_| ExecutionError::Expression(format!("cannot cast '{}' to real", text))),
            other => Err(ExecutionError::Expression(format!(
                "cannot cast {:?} to real",
                other
            ))),
        },
        DataType::Text => Ok(Value::String(value_to_string(&value)?)),
        DataType::Boolean => match value {
            Value::Boolean(flag) => Ok(Value::Boolean(flag)),
            Value::Integer(number) => Ok(Value::Boolean(number != 0)),
            Value::Timestamp(number) => Ok(Value::Boolean(number != 0)),
            Value::Float(number) => Ok(Value::Boolean(number != 0.0)),
            Value::String(text) => {
                let normalized = text.trim().to_lowercase();
                match normalized.as_str() {
                    "true" => Ok(Value::Boolean(true)),
                    "false" => Ok(Value::Boolean(false)),
                    _ => Err(ExecutionError::Expression(format!(
                        "cannot cast '{}' to boolean",
                        text
                    ))),
                }
            }
            other => Err(ExecutionError::Expression(format!(
                "cannot cast {:?} to boolean",
                other
            ))),
        },
        DataType::Timestamp => match value {
            Value::Timestamp(number) => Ok(Value::Timestamp(number)),
            Value::Integer(number) => Ok(Value::Timestamp(number)),
            Value::Float(number) => Ok(Value::Timestamp(number as i64)),
            Value::String(text) => text.parse::<i64>().map(Value::Timestamp).map_err(|_| {
                ExecutionError::Expression(format!("cannot cast '{}' to timestamp", text))
            }),
            other => Err(ExecutionError::Expression(format!(
                "cannot cast {:?} to timestamp",
                other
            ))),
        },
    }
}

fn value_to_string(value: &Value) -> ExecutionResult<String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Integer(number) => Ok(number.to_string()),
        Value::Timestamp(number) => Ok(number.to_string()),
        Value::Float(number) => Ok(number.to_string()),
        Value::Boolean(flag) => Ok(flag.to_string()),
        Value::Null => Ok("NULL".to_string()),
    }
}
