use db::engine::Engine;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use wal::Transaction;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Mutex<Engine>>,
    pub transactions: Arc<Mutex<HashMap<String, Arc<Mutex<Transaction>>>>>,
}
