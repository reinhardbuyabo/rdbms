use crate::expr::LiteralValue;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Timestamp(i64),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

impl From<LiteralValue> for Value {
    fn from(value: LiteralValue) -> Self {
        match value {
            LiteralValue::Null => Value::Null,
            LiteralValue::Integer(number) => Value::Integer(number),
            LiteralValue::Float(number) => Value::Float(number),
            LiteralValue::String(text) => Value::String(text),
            LiteralValue::Boolean(flag) => Value::Boolean(flag),
        }
    }
}

impl From<&LiteralValue> for Value {
    fn from(value: &LiteralValue) -> Self {
        match value {
            LiteralValue::Null => Value::Null,
            LiteralValue::Integer(number) => Value::Integer(*number),
            LiteralValue::Float(number) => Value::Float(*number),
            LiteralValue::String(text) => Value::String(text.clone()),
            LiteralValue::Boolean(flag) => Value::Boolean(*flag),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tuple {
    values: Vec<Value>,
}

impl Tuple {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> &[Value] {
        &self.values
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    pub fn concat(&self, other: &Tuple) -> Tuple {
        let mut values = Vec::with_capacity(self.values.len() + other.values.len());
        values.extend(self.values.iter().cloned());
        values.extend(other.values.iter().cloned());
        Tuple::new(values)
    }
}
