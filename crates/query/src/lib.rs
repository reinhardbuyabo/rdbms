pub mod execution;
pub mod expr;
pub mod logical_plan;
pub mod parser;
pub mod planner;
pub mod schema;

pub use execution::{Catalog, Executor, PhysicalPlanner, TableHeap, TableInfo, Tuple, Value};
pub use expr::{BinaryOperator, Expr, LiteralValue, UnaryOperator};
pub use logical_plan::{
    AggregateExpr, AggregateFunction, Assignment, JoinType, LogicalPlan, SortExpr,
};
pub use parser::SqlParser;
pub use planner::LogicalPlanner;
pub use schema::{ColumnDef, DataType, DefaultValue, Field, Schema, TableSchema};

use anyhow::Result;

pub fn sql_to_logical_plan(sql: &str) -> Result<LogicalPlan> {
    let parser = SqlParser::new();
    let stmt = parser.parse_one(sql)?;
    let mut planner = LogicalPlanner::new();
    planner.plan_statement(stmt)
}

pub fn explain_sql(sql: &str) -> Result<String> {
    let plan = sql_to_logical_plan(sql)?;
    Ok(plan.explain())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_api_simple_select() {
        let plan = sql_to_logical_plan("SELECT * FROM users").unwrap();
        assert!(matches!(plan, LogicalPlan::Project { .. }));
    }
    #[test]
    fn test_api_explain() {
        let explanation = explain_sql("SELECT * FROM users WHERE age > 18").unwrap();
        assert!(explanation.contains("Filter"));
        assert!(explanation.contains("Scan"));
    }

    #[test]
    fn test_dot_exporter() {
        let plan = sql_to_logical_plan("SELECT * FROM users WHERE age > 18").unwrap();
        let dot = plan.to_dot();
        assert!(dot.contains("digraph"));
        assert!(dot.contains("n1"));
        assert!(dot.contains("->"));
        println!("DOT output:\n{}", dot);
    }
}
