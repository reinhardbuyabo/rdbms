use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex, MutexGuard};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TxnId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LockKey {
    Page(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlockPolicy {
    Timeout(Duration),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockError {
    DeadlockTimeout,
    LockAlreadyHeld,
}

pub type LockResult<T> = Result<T, LockError>;

#[derive(Debug)]
struct LockRequest {
    txn_id: TxnId,
    mode: LockMode,
}

#[derive(Debug, Default)]
struct LockState {
    mode: Option<LockMode>,
    holders: HashSet<TxnId>,
    waiters: VecDeque<LockRequest>,
}

#[derive(Debug, Default)]
struct LockManagerState {
    locks: HashMap<LockKey, LockState>,
    held_keys: HashMap<TxnId, HashSet<LockKey>>,
}

pub struct LockManager {
    state: Mutex<LockManagerState>,
    condvar: Condvar,
    policy: DeadlockPolicy,
}

impl LockManager {
    pub fn new(policy: DeadlockPolicy) -> Self {
        Self {
            state: Mutex::new(LockManagerState::default()),
            condvar: Condvar::new(),
            policy,
        }
    }

    pub fn lock_shared(&self, txn_id: TxnId, key: LockKey) -> LockResult<()> {
        self.lock(txn_id, key, LockMode::Shared)
    }

    pub fn lock_exclusive(&self, txn_id: TxnId, key: LockKey) -> LockResult<()> {
        self.lock(txn_id, key, LockMode::Exclusive)
    }

    pub fn unlock_all(&self, txn_id: TxnId) {
        let mut state = self.state.lock();
        let Some(keys) = state.held_keys.remove(&txn_id) else {
            return;
        };
        for key in keys {
            let lock_state = state.locks.get_mut(&key).expect("lock state exists");
            lock_state.holders.remove(&txn_id);
            if lock_state.holders.is_empty() {
                lock_state.mode = None;
            }
        }
        self.process_waiters(&mut state);
        self.condvar.notify_all();
    }

    pub fn held_keys_for(&self, txn_id: TxnId) -> Vec<LockKey> {
        let state = self.state.lock();
        state
            .held_keys
            .get(&txn_id)
            .map(|keys| keys.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn lock(&self, txn_id: TxnId, key: LockKey, mode: LockMode) -> LockResult<()> {
        let mut state = self.state.lock();
        if self.holds_lock(&state, txn_id, &key, mode) {
            return Ok(());
        }
        let deadline = self.deadline();
        loop {
            let should_wait;
            {
                let lock_state = state.locks.entry(key.clone()).or_default();
                if self.can_grant(lock_state, txn_id, mode) && lock_state.waiters.is_empty() {
                    lock_state.mode = Some(mode);
                    lock_state.holders.insert(txn_id);
                    state
                        .held_keys
                        .entry(txn_id)
                        .or_default()
                        .insert(key.clone());
                    return Ok(());
                }
                if !lock_state
                    .waiters
                    .iter()
                    .any(|waiter| waiter.txn_id == txn_id)
                {
                    lock_state.waiters.push_back(LockRequest { txn_id, mode });
                }
                should_wait = true;
            }
            if should_wait {
                state = self.wait_for_lock(state, deadline)?;
            }
        }
    }

    fn wait_for_lock<'a>(
        &self,
        mut state: MutexGuard<'a, LockManagerState>,
        deadline: Option<Instant>,
    ) -> LockResult<MutexGuard<'a, LockManagerState>> {
        match deadline {
            Some(deadline) => {
                let now = Instant::now();
                if now >= deadline {
                    return Err(LockError::DeadlockTimeout);
                }
                let remaining = deadline.saturating_duration_since(now);
                let timeout = self.condvar.wait_for(&mut state, remaining);
                if timeout.timed_out() {
                    return Err(LockError::DeadlockTimeout);
                }
                Ok(state)
            }
            None => {
                self.condvar.wait(&mut state);
                Ok(state)
            }
        }
    }

    fn deadline(&self) -> Option<Instant> {
        match self.policy {
            DeadlockPolicy::Timeout(duration) => Some(Instant::now() + duration),
        }
    }

    fn can_grant(&self, lock_state: &LockState, txn_id: TxnId, mode: LockMode) -> bool {
        match lock_state.mode {
            None => true,
            Some(LockMode::Shared) => {
                mode == LockMode::Shared
                    || (lock_state.holders.len() == 1 && lock_state.holders.contains(&txn_id))
            }
            Some(LockMode::Exclusive) => lock_state.holders.contains(&txn_id),
        }
    }

    fn holds_lock(
        &self,
        state: &LockManagerState,
        txn_id: TxnId,
        key: &LockKey,
        mode: LockMode,
    ) -> bool {
        let Some(lock_state) = state.locks.get(key) else {
            return false;
        };
        if !lock_state.holders.contains(&txn_id) {
            return false;
        }
        matches!(
            (lock_state.mode, mode),
            (Some(LockMode::Exclusive), _) | (Some(LockMode::Shared), LockMode::Shared)
        )
    }

    fn process_waiters(&self, state: &mut LockManagerState) {
        let keys: Vec<LockKey> = state.locks.keys().cloned().collect();
        for key in keys {
            let lock_state = state.locks.get_mut(&key).expect("lock state exists");
            if lock_state.holders.is_empty() {
                lock_state.mode = None;
            }
            self.promote_waiters(state, key.clone());
        }
    }

    fn promote_waiters(&self, state: &mut LockManagerState, key: LockKey) {
        let lock_state = state.locks.get_mut(&key).expect("lock state exists");
        let mut promoted_any = false;
        while let Some(request) = lock_state.waiters.front() {
            if !self.can_grant(lock_state, request.txn_id, request.mode) {
                break;
            }
            let request = lock_state.waiters.pop_front().expect("waiter exists");
            lock_state.mode = Some(request.mode);
            lock_state.holders.insert(request.txn_id);
            state
                .held_keys
                .entry(request.txn_id)
                .or_default()
                .insert(key.clone());
            promoted_any = true;
            if request.mode == LockMode::Exclusive {
                break;
            }
        }
        if promoted_any {
            self.condvar.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;
    
    mod lock_tests;
    mod integration_tests;
}

    fn manager() -> LockManager {
        LockManager::new(DeadlockPolicy::Timeout(Duration::from_millis(200)))
    }

    #[test]
    fn shared_shared_is_compatible() {
        let manager = manager();
        let txn1 = TxnId(1);
        let txn2 = TxnId(2);
        let key = LockKey::Page(42);
        assert!(manager.lock_shared(txn1, key.clone()).is_ok());
        assert!(manager.lock_shared(txn2, key.clone()).is_ok());
        let held = manager.held_keys_for(txn1);
        assert_eq!(held, vec![key]);
    }

    #[test]
    fn exclusive_blocks_shared() {
        let manager = Arc::new(manager());
        let key = LockKey::Page(1);
        manager.lock_exclusive(TxnId(1), key.clone()).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let manager_clone = Arc::clone(&manager);
        let barrier_clone = Arc::clone(&barrier);
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            manager_clone.lock_shared(TxnId(2), key)
        });
        barrier.wait();
        thread::sleep(Duration::from_millis(50));
        manager.unlock_all(TxnId(1));
        let result = handle.join().unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn exclusive_blocks_exclusive() {
        let manager = Arc::new(manager());
        let key = LockKey::Page(7);
        manager.lock_exclusive(TxnId(1), key.clone()).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let manager_clone = Arc::clone(&manager);
        let barrier_clone = Arc::clone(&barrier);
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            manager_clone.lock_exclusive(TxnId(2), key)
        });
        barrier.wait();
        thread::sleep(Duration::from_millis(50));
        manager.unlock_all(TxnId(1));
        let result = handle.join().unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn shared_blocks_exclusive_timeout() {
        let manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            Duration::from_millis(50),
        )));
        let key = LockKey::Page(9);
        manager.lock_shared(TxnId(1), key.clone()).unwrap();
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || manager_clone.lock_exclusive(TxnId(2), key));
        let result = handle.join().unwrap();
        assert_eq!(result, Err(LockError::DeadlockTimeout));
    }

    #[test]
    fn upgrade_shared_to_exclusive() {
        let manager = manager();
        let key = LockKey::Page(11);
        let txn = TxnId(1);
        manager.lock_shared(txn, key.clone()).unwrap();
        manager.lock_exclusive(txn, key.clone()).unwrap();
        assert_eq!(manager.held_keys_for(txn), vec![key]);
    }

    #[test]
    fn unlock_all_releases_keys() {
        let manager = manager();
        let txn = TxnId(1);
        let keys = vec![LockKey::Page(1), LockKey::Page(2), LockKey::Page(3)];
        for key in &keys {
            manager.lock_exclusive(txn, key.clone()).unwrap();
        }
        manager.unlock_all(txn);
        assert!(manager.held_keys_for(txn).is_empty());
    }
