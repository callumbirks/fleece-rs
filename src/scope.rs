use std::ops::Deref;
use std::ptr;
use std::ptr::NonNull;
use std::sync::{atomic::AtomicBool, Arc, OnceLock, RwLock, Weak};

use rangemap::RangeMap;

use crate::{sharedkeys::SharedKeys, Value};

pub struct Scope {
    pub shared_keys: Option<Arc<SharedKeys>>,
    pub data: Weak<[u8]>,
    pub alloced_data: Option<Arc<[u8]>>,
    root: *const Value,
    registered: AtomicBool,
}

impl Scope {
    pub fn find_shared_keys(containing_data: *const u8) -> Option<Arc<SharedKeys>> {
        Scope::containing(containing_data)
            .map(|s| s.shared_keys.clone())
            .flatten()
    }

    pub fn new_alloced(
        data: impl Into<Arc<[u8]>>,
        shared_keys: Option<Arc<SharedKeys>>,
    ) -> Option<Arc<Self>> {
        let scope_map = SCOPE_MAP.get_or_init(|| RwLock::new(RangeMap::new()));
        let mut scope_map = scope_map.write().ok()?;

        let alloced_data = data.into();
        let data = Arc::downgrade(&alloced_data);

        let start = alloced_data.as_ptr() as usize;
        let end = alloced_data.as_ptr() as usize + alloced_data.len();

        let root = Value::from_bytes(&alloced_data).map_or_else(
            |_| ptr::slice_from_raw_parts(ptr::null::<u8>(), 0) as *const Value,
            |v| v as *const Value,
        );

        let scope = Arc::new(Scope {
            shared_keys,
            data,
            alloced_data: Some(alloced_data),
            root,
            registered: AtomicBool::new(true),
        });

        // TODO: Figure out how to protect against overlaps
        scope_map.insert(start..end, ScopeEntry(Arc::downgrade(&scope)));

        Some(scope)
    }

    /// Return a `ScopedValue` containing the root Value belonging to this Scope.
    pub fn root(&self) -> Option<ScopedValue> {
        if let Some(data) = self.data() {
            Some(ScopedValue {
                data,
                value: NonNull::from(unsafe { &*self.root }),
            })
        } else {
            None
        }
    }

    pub fn data(&self) -> Option<Arc<[u8]>> {
        if let Some(alloced_data) = &self.alloced_data {
            Some(alloced_data.clone())
        } else {
            self.data.upgrade()
        }
    }

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

/// Holds a reference to a `Value` and also retains the data which contains the Value.
pub struct ScopedValue {
    data: Arc<[u8]>,
    value: NonNull<Value>,
}

impl ScopedValue {
    pub fn value(&self) -> &Value {
        unsafe {
            self.value.as_ref()
        }
    }
}

impl PartialEq for Scope {
    fn eq(&self, other: &Self) -> bool {
        let Some(self_data) = self.data.upgrade() else {
            return false;
        };
        let Some(other_data) = other.data.upgrade() else {
            return false;
        };
        self_data.as_ptr().eq(&other_data.as_ptr())
    }
}

impl Eq for Scope {}

#[derive(Clone)]
struct ScopeEntry(Weak<Scope>);

impl PartialEq for ScopeEntry {
    fn eq(&self, other: &Self) -> bool {
        let Some(self_scope) = self.0.upgrade() else {
            return false;
        };
        let Some(other_scope) = other.0.upgrade() else {
            return false;
        };
        self_scope.eq(&other_scope)
    }
}

impl Eq for ScopeEntry {}

impl Deref for ScopeEntry {
    type Target = Weak<Scope>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

unsafe impl Send for Scope {}
unsafe impl Sync for Scope {}

static SCOPE_MAP: OnceLock<RwLock<RangeMap<usize, ScopeEntry>>> = OnceLock::new();
