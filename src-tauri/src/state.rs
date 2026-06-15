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
        if let Some(previous) = map.insert(task_id.to_string(), cancelled.clone()) {
            previous.store(true, Ordering::SeqCst);
        }
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

    pub fn remove_if_current(&self, task_id: &str, token: &Arc<AtomicBool>) -> bool {
        let mut map = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        let is_current = map
            .get(task_id)
            .map(|current| Arc::ptr_eq(current, token))
            .unwrap_or(false);
        if is_current {
            map.remove(task_id);
        }
        is_current
    }
}

#[derive(Clone)]
pub struct AppState {
    pub core: Arc<ReaderCore>,
    pub tasks: TaskRegistry,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registering_existing_task_cancels_previous_token() {
        let registry = TaskRegistry::default();

        let first = registry.register("source-task");
        let second = registry.register("source-task");

        assert!(first.load(Ordering::SeqCst));
        assert!(!second.load(Ordering::SeqCst));
        assert!(registry.cancel("source-task"));
        assert!(second.load(Ordering::SeqCst));
    }

    #[test]
    fn old_task_completion_does_not_remove_replacement_token() {
        let registry = TaskRegistry::default();

        let first = registry.register("source-task");
        let second = registry.register("source-task");

        assert!(!registry.remove_if_current("source-task", &first));
        assert!(registry.cancel("source-task"));
        assert!(second.load(Ordering::SeqCst));
    }

    #[test]
    fn current_task_completion_removes_token() {
        let registry = TaskRegistry::default();

        let token = registry.register("source-task");

        assert!(registry.remove_if_current("source-task", &token));
        assert!(!registry.cancel("source-task"));
    }
}
