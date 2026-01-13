use crate::schema::DataType;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Column {
        table: Option<String>,
        name: String,
    },
    Literal(LiteralValue),
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expr>,
    },
    Function {
        name: String,
        args: Vec<Expr>,
    },
    Wildcard,
    QualifiedWildcard {
        table: String,
    },
    Cast {
        expr: Box<Expr>,
        target_type: DataType,
    },
    IsNull {
        expr: Box<Expr>,
        negated: bool,
    },
    Between {
        expr: Box<Expr>,
        low: Box<Expr>,
        high: Box<Expr>,
        negated: bool,
    },
    In {
        expr: Box<Expr>,
        list: Vec<Expr>,
        negated: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Null,
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
}

impl fmt::Display for LiteralValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiteralValue::Null => write!(f, "NULL"),
            LiteralValue::Integer(n) => write!(f, "{}", n),
            LiteralValue::Float(n) => write!(f, "{}", n),
            LiteralValue::String(s) => write!(f, "'{}'", s),
            LiteralValue::Boolean(b) => write!(f, "{}", b),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
    Concat,
    Like,
    NotLike,
}

impl fmt::Display for BinaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BinaryOperator::Plus => "+",
            BinaryOperator::Minus => "-",
            BinaryOperator::Multiply => "*",
            BinaryOperator::Divide => "/",
            BinaryOperator::Modulo => "%",
            BinaryOperator::Eq => "=",
            BinaryOperator::NotEq => "!=",
            BinaryOperator::Lt => "<",
            BinaryOperator::LtEq => "<=",
            BinaryOperator::Gt => ">",
            BinaryOperator::GtEq => ">=",
            BinaryOperator::And => "AND",
            BinaryOperator::Or => "OR",
            BinaryOperator::Concat => "||",
            BinaryOperator::Like => "LIKE",
            BinaryOperator::NotLike => "NOT LIKE",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Minus,
    Plus,
}

impl fmt::Display for UnaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            UnaryOperator::Not => "NOT",
            UnaryOperator::Minus => "-",
            UnaryOperator::Plus => "+",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Column { table, name } => {
                if let Some(t) = table {
                    write!(f, "{}.{}", t, name)
                } else {
                    write!(f, "{}", name)
                }
            }
            Expr::Literal(lit) => write!(f, "{}", lit),
            Expr::BinaryOp { left, op, right } => {
                write!(f, "({} {} {})", left, op, right)
            }
            Expr::UnaryOp { op, expr } => {
                write!(f, "({} {})", op, expr)
            }
            Expr::Function { name, args } => {
                write!(f, "{}(", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Expr::Wildcard => write!(f, "*"),
            Expr::QualifiedWildcard { table } => write!(f, "{}.*", table),
            Expr::Cast { expr, target_type } => {
                write!(f, "CAST({} AS {:?})", expr, target_type)
            }
            Expr::IsNull { expr, negated } => {
                if *negated {
                    write!(f, "{} IS NOT NULL", expr)
                } else {
                    write!(f, "{} IS NULL", expr)
                }
            }
            Expr::Between {
                expr,
                low,
                high,
                negated,
            } => {
                if *negated {
                    write!(f, "{} NOT BETWEEN {} AND {}", expr, low, high)
                } else {
                    write!(f, "{} BETWEEN {} AND {}", expr, low, high)
                }
            }
            Expr::In {
                expr,
                list,
                negated,
            } => {
                if *negated {
                    write!(f, "{} NOT IN (", expr)?;
                } else {
                    write!(f, "{} IN (", expr)?;
                }
                for (i, item) in list.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, ")")
            }
        }
    }
}
