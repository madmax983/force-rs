use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

struct TokenState {
    token: Option<Arc<String>>,
    clear_count: u64,
}

struct TokenManager {
    state: Arc<RwLock<TokenState>>,
    refresh_lock: Mutex<()>,
}
