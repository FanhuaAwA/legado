use reader_core::ReaderCore;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct TaskRegistry {
    tokens: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl TaskRegistry {
    pub fn register(&self, task_id: &str) -> Arc<AtomicBool> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut map = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(task_id.to_string(), cancelled.clone());
        cancelled
    }

    pub fn cancel(&self, task_id: &str) -> bool {
        let mut map = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cancelled) = map.remove(task_id) {
            cancelled.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn remove(&self, task_id: &str) {
        let mut map = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(task_id);
    }
}

#[derive(Clone)]
pub struct AppState {
    pub core: Arc<ReaderCore>,
    pub tasks: TaskRegistry,
}
