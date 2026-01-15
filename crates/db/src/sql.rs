#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitResult {
    pub statements: Vec<String>,
    pub remainder: String,
    pub in_string: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    Normal,
    SingleQuote,
    DoubleQuote,
    LineComment,
    BlockComment,
}

pub fn split_statements(input: &str) -> SplitResult {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut state = ParserState::Normal;
    let mut statement_start = 0;
    let mut iter = input.char_indices().peekable();

    while let Some((idx, ch)) = iter.next() {
        match state {
            ParserState::Normal => match ch {
                '\'' => {
                    current.push(ch);
                    state = ParserState::SingleQuote;
                }
                '"' => {
                    current.push(ch);
                    state = ParserState::DoubleQuote;
                }
                '-' => {
                    if let Some((_, next)) = iter.peek()
                        && *next == '-'
                    {
                        iter.next();
                        state = ParserState::LineComment;
                        continue;
                    }
                    current.push(ch);
                }
                '/' => {
                    if let Some((_, next)) = iter.peek()
                        && *next == '*'
                    {
                        iter.next();
                        state = ParserState::BlockComment;
                        continue;
                    }
                    current.push(ch);
                }
                ';' => {
                    let statement = current.trim();
                    if !statement.is_empty() {
                        statements.push(statement.to_string());
                    }
                    current.clear();
                    statement_start = idx + ch.len_utf8();
                }
                _ => current.push(ch),
            },
            ParserState::SingleQuote => {
                if ch == '\'' {
                    if let Some((_, next)) = iter.peek()
                        && *next == '\''
                    {
                        current.push(ch);
                        current.push(*next);
                        iter.next();
                        continue;
                    }
                    current.push(ch);
                    state = ParserState::Normal;
                } else {
                    current.push(ch);
                }
            }
            ParserState::DoubleQuote => {
                if ch == '"' {
                    if let Some((_, next)) = iter.peek()
                        && *next == '"'
                    {
                        current.push(ch);
                        current.push(*next);
                        iter.next();
                        continue;
                    }
                    current.push(ch);
                    state = ParserState::Normal;
                } else {
                    current.push(ch);
                }
            }
            ParserState::LineComment => {
                if ch == '\n' {
                    current.push(ch);
                    state = ParserState::Normal;
                }
            }
            ParserState::BlockComment => {
                if ch == '*'
                    && let Some((_, next)) = iter.peek()
                    && *next == '/'
                {
                    iter.next();
                    push_space_if_needed(&mut current);
                    state = ParserState::Normal;
                }
            }
        }
    }

    let remainder_raw = &input[statement_start..];
    let final_state = if state == ParserState::LineComment {
        ParserState::Normal
    } else {
        state
    };
    let needs_remainder = !current.trim().is_empty()
        || matches!(
            final_state,
            ParserState::SingleQuote | ParserState::DoubleQuote | ParserState::BlockComment
        );
    let remainder = if !needs_remainder {
        String::new()
    } else if final_state == ParserState::BlockComment {
        remainder_raw.to_string()
    } else {
        current.to_string()
    };

    SplitResult {
        statements,
        remainder,
        in_string: matches!(
            final_state,
            ParserState::SingleQuote | ParserState::DoubleQuote | ParserState::BlockComment
        ),
    }
}

fn push_space_if_needed(current: &mut String) {
    let needs_space = current
        .chars()
        .last()
        .map(|ch| !ch.is_whitespace())
        .unwrap_or(false);
    if needs_space {
        current.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_multiple_statements() {
        let result = split_statements("SELECT 1; SELECT 2;");
        assert_eq!(result.statements, vec!["SELECT 1", "SELECT 2"]);
        assert!(result.remainder.is_empty());
    }

    #[test]
    fn ignores_semicolons_in_strings() {
        let result = split_statements("INSERT INTO t VALUES ('a; b');");
        assert_eq!(result.statements, vec!["INSERT INTO t VALUES ('a; b')"]);
        assert!(result.remainder.is_empty());
    }

    #[test]
    fn keeps_incomplete_statement() {
        let result = split_statements("SELECT * FROM users");
        assert!(result.statements.is_empty());
        assert_eq!(result.remainder, "SELECT * FROM users");
        assert!(!result.in_string);
    }

    #[test]
    fn tracks_open_string() {
        let result = split_statements("SELECT 'unterminated");
        assert!(result.statements.is_empty());
        assert!(result.in_string);
    }

    #[test]
    fn line_comment_after_statement() {
        let result = split_statements("SELECT 1; -- comment;");
        assert_eq!(result.statements, vec!["SELECT 1"]);
        assert!(result.remainder.is_empty());
    }

    #[test]
    fn line_comment_only_line() {
        let result = split_statements("-- comment only\n");
        assert!(result.statements.is_empty());
        assert!(result.remainder.is_empty());
    }

    #[test]
    fn line_comment_before_semicolon() {
        let result = split_statements("SELECT 1 -- comment\n;");
        assert_eq!(result.statements, vec!["SELECT 1"]);
        assert!(result.remainder.is_empty());
    }

    #[test]
    fn line_comment_trailing_no_newline() {
        let result = split_statements("SELECT 1; -- comment");
        assert_eq!(result.statements, vec!["SELECT 1"]);
        assert!(result.remainder.is_empty());
    }

    #[test]
    fn block_comment_with_semicolon() {
        let result = split_statements("SELECT 1 /* comment; */; SELECT 2;");
        assert_eq!(result.statements, vec!["SELECT 1", "SELECT 2"]);
        assert!(result.remainder.is_empty());
    }

    #[test]
    fn multiline_block_comment() {
        let result = split_statements("SELECT 1 /* comment\n; still comment */;");
        assert_eq!(result.statements, vec!["SELECT 1"]);
        assert!(result.remainder.is_empty());
    }

    #[test]
    fn unterminated_block_comment_keeps_remainder() {
        let input = "SELECT 1 /* comment";
        let result = split_statements(input);
        assert!(result.statements.is_empty());
        assert_eq!(result.remainder, input);
        assert!(result.in_string);
    }

    #[test]
    fn block_comment_adjacent_tokens() {
        let result = split_statements("SELECT/*comment*/1;");
        assert_eq!(result.statements, vec!["SELECT 1"]);
        assert!(result.remainder.is_empty());
    }

    #[test]
    fn comment_markers_inside_strings() {
        let result = split_statements("SELECT '-- not a comment;';");
        assert_eq!(result.statements, vec!["SELECT '-- not a comment;'"]);
        assert!(result.remainder.is_empty());
    }

    #[test]
    fn mixed_comments_and_multiple_statements() {
        let sql = "SELECT 1; /* comment */ SELECT 2; -- end\nSELECT 3;";
        let result = split_statements(sql);
        assert_eq!(result.statements, vec!["SELECT 1", "SELECT 2", "SELECT 3"]);
        assert!(result.remainder.is_empty());
    }

    #[test]
    fn comment_tokens_in_escaped_strings() {
        let result = split_statements("INSERT INTO t VALUES ('--', '/* */', 'it''s fine');");
        assert_eq!(
            result.statements,
            vec!["INSERT INTO t VALUES ('--', '/* */', 'it''s fine')"]
        );
        assert!(result.remainder.is_empty());
    }
}
