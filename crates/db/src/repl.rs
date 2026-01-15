use std::fs;

use anyhow::{Context, Result};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

use crate::commands::{MetaCommand, parse_meta_command};
use crate::engine::{Engine, schema_to_description, tables_to_output};
use crate::history::resolve_history_path;
use crate::printer::print_output;
use crate::sql::split_statements;

const PRIMARY_PROMPT: &str = "rdbms> ";
const CONTINUATION_PROMPT: &str = "...> ";

pub fn run_repl(engine: &mut Engine) -> Result<()> {
    let history_path = resolve_history_path();
    if let Some(parent) = history_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).context("create history directory")?;
    }

    let mut editor = DefaultEditor::new().context("initialize line editor")?;
    let _ = editor.load_history(&history_path);

    let mut buffer = String::new();

    loop {
        let prompt = if buffer.trim().is_empty() {
            PRIMARY_PROMPT
        } else {
            CONTINUATION_PROMPT
        };
        let line = match editor.readline(prompt) {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => {
                buffer.clear();
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => break,
            Err(err) => return Err(err.into()),
        };

        if buffer.is_empty() && line.trim().is_empty() {
            continue;
        }

        buffer.push_str(&line);
        buffer.push('\n');

        let input = buffer.clone();
        let split = split_statements(&buffer);
        let mut should_exit = false;

        for statement in split.statements {
            match parse_meta_command(&statement) {
                Some(command) => {
                    if handle_meta_command(engine, command)? {
                        should_exit = true;
                        break;
                    }
                }
                None => match engine.execute_sql(&statement) {
                    Ok(output) => print_output(&output),
                    Err(err) => eprintln!("Error: {}", err),
                },
            }
        }

        if should_exit {
            break;
        }

        if split.remainder.is_empty() && !split.in_string {
            if !input.trim().is_empty() {
                let _ = editor.add_history_entry(input.trim());
            }
            buffer.clear();
        } else {
            buffer = split.remainder;
        }
    }

    let _ = editor.save_history(&history_path);
    Ok(())
}

fn handle_meta_command(engine: &Engine, command: MetaCommand) -> Result<bool> {
    match command {
        MetaCommand::Quit => Ok(true),
        MetaCommand::Help => {
            print_help();
            Ok(false)
        }
        MetaCommand::Tables => {
            let tables = engine.list_tables();
            print_output(&tables_to_output(&tables));
            Ok(false)
        }
        MetaCommand::Schema { table } => {
            match engine.table_schema(&table) {
                Some(schema) => print_output(&schema_to_description(&schema)),
                None => eprintln!("Error: table {} not found", table),
            }
            Ok(false)
        }
    }
}

fn print_help() {
    println!("Commands:");
    println!("  \\q, exit, quit    Exit the REPL");
    println!("  \\help            Show this message");
    println!("  \\tables          List tables");
    println!("  \\schema <table>  Show table schema");
    println!("\nEnter SQL statements terminated by ';'.");
}
