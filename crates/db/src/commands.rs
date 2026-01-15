use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetaCommand {
    Quit,
    Help,
    Tables,
    Schema { table: String },
}

impl fmt::Display for MetaCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetaCommand::Quit => write!(f, "quit"),
            MetaCommand::Help => write!(f, "help"),
            MetaCommand::Tables => write!(f, "tables"),
            MetaCommand::Schema { table } => write!(f, "schema {}", table),
        }
    }
}

pub fn parse_meta_command(input: &str) -> Option<MetaCommand> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.trim_end_matches(';').trim();
    let lower = normalized.to_lowercase();

    match lower.as_str() {
        "\\q" | "\\quit" | ".quit" | ".exit" | "quit" | "exit" => {
            return Some(MetaCommand::Quit);
        }
        "\\help" | ".help" | "help" => return Some(MetaCommand::Help),
        "\\tables" | ".tables" => return Some(MetaCommand::Tables),
        _ => {}
    }

    if let Some(rest) = lower.strip_prefix("\\schema ") {
        return Some(MetaCommand::Schema {
            table: rest.trim().to_string(),
        });
    }
    if let Some(rest) = lower.strip_prefix(".schema ") {
        return Some(MetaCommand::Schema {
            table: rest.trim().to_string(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_meta_commands() {
        assert_eq!(parse_meta_command("\\q"), Some(MetaCommand::Quit));
        assert_eq!(parse_meta_command(".exit"), Some(MetaCommand::Quit));
        assert_eq!(parse_meta_command("exit"), Some(MetaCommand::Quit));
        assert_eq!(parse_meta_command("\\help"), Some(MetaCommand::Help));
        assert_eq!(parse_meta_command(".tables"), Some(MetaCommand::Tables));
    }

    #[test]
    fn parses_schema_command() {
        assert_eq!(
            parse_meta_command("\\schema users"),
            Some(MetaCommand::Schema {
                table: "users".to_string()
            })
        );
    }
}
