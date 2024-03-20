use std::sync::{atomic::AtomicBool, Arc, OnceLock, RwLock, Weak};

use rangemap::RangeMap;

use crate::{sharedkeys::SharedKeys, Value};

pub struct Scope {
    shared_keys: Arc<SharedKeys>,
    data: Weak<[u8]>,
    alloced_data: Arc<[u8]>,
    root: Weak<Value>,
    registered: AtomicBool,
}

impl Scope {
    fn containing(data: *const u8) -> Option<Arc<Scope>> {
        let scope_map = SCOPE_MAP.get_or_init(|| RwLock::new(RangeMap::new()));
        let Ok(scope_map) = scope_map.read() else {
            return None;
        };

        // Get the Scope which covers the range including data
        if let Some(scope_weak) = scope_map.get(&(data as usize)) {
            // Scope is held with a weak pointer, so we need to upgrade it to an Arc, which checks
            // that the Scope hasn't been deallocated yet.
            if let Some(scope) = scope_weak.upgrade() {
                return Some(scope);
            }
        }

        None
    }
}

static SCOPE_MAP: OnceLock<RwLock<RangeMap<usize, Weak<Scope>>>> = OnceLock::new();
