use std::env;
use std::path::PathBuf;

const PROJECT_DIR: &str = "rdbms";

pub fn resolve_history_path() -> PathBuf {
    if let Ok(state_dir) = env::var("XDG_STATE_HOME") {
        return PathBuf::from(state_dir).join(PROJECT_DIR).join("history");
    }
    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("state")
            .join(PROJECT_DIR)
            .join("history");
    }
    PathBuf::from(".rdbms_history")
}
