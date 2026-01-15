use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use db::engine::Engine;
use db::repl::run_repl;

#[derive(Parser, Debug)]
#[command(name = "rdbms", about = "Interactive SQL REPL")]
struct Args {
    #[arg(long, value_name = "PATH", default_value = "data")]
    db: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if let Some(parent) = args.db.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).context("create db directory")?;
    }

    println!("RDBMS REPL v{}", env!("CARGO_PKG_VERSION"));
    println!("Using database file: {}", args.db.display());

    let mut engine = Engine::new(&args.db)?;
    run_repl(&mut engine)
}
