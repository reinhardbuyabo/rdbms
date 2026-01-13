use sqlparser::ast::Statement;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::{Parser, ParserError};

pub struct SqlParser {
    dialect: GenericDialect,
}

impl SqlParser {
    pub fn new() -> Self {
        Self {
            dialect: GenericDialect {},
        }
    }
    pub fn parse(&self, sql: &str) -> Result<Vec<Statement>, ParserError> {
        Parser::parse_sql(&self.dialect, sql)
    }
    pub fn parse_one(&self, sql: &str) -> Result<Statement, ParserError> {
        let statements = self.parse(sql)?;
        if statements.is_empty() {
            return Err(ParserError::ParserError(
                "No SQL statement found".to_string(),
            ));
        }
        if statements.len() > 1 {
            return Err(ParserError::ParserError(
                "Expected single statement, found multiple".to_string(),
            ));
        }
        Ok(statements.into_iter().next().unwrap())
    }
}

impl Default for SqlParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_simple_select() {
        let parser = SqlParser::new();
        let result = parser.parse_one("SELECT * FROM users");
        assert!(result.is_ok());
    }
    #[test]
    fn test_parse_invalid_sql() {
        let parser = SqlParser::new();
        let result = parser.parse_one("SELECT FROM WHERE");
        assert!(result.is_err());
    }
    #[test]
    fn test_parse_join() {
        let parser = SqlParser::new();
        let result =
            parser.parse_one("SELECT * FROM Event e JOIN TicketType t ON e.id = t.eventId");
        assert!(result.is_ok());
    }
    #[test]
    fn test_parse_multiple_statements() {
        let parser = SqlParser::new();
        let result = parser.parse_one("SELECT 1; SELECT 2;");
        assert!(result.is_err());
    }
    #[test]
    fn test_parse_empty() {
        let parser = SqlParser::new();
        let result = parser.parse_one("");
        assert!(result.is_err());
    }
}
