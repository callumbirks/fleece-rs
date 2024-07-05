use std::ops::Deref;
use std::ptr;
use std::ptr::NonNull;
use std::sync::{atomic::AtomicBool, Arc, OnceLock, RwLock, Weak};

use rangemap::RangeMap;

use crate::{sharedkeys::SharedKeys, Value};

pub struct Scope {
    pub shared_keys: Option<Arc<SharedKeys>>,
    weak_data: Weak<[u8]>,
    strong_data: Option<Arc<[u8]>>,
    root: *const Value,
    registered: AtomicBool,
}

impl Scope {
    pub fn find_shared_keys(containing_data: *const u8) -> Option<Arc<SharedKeys>> {
        Scope::containing(containing_data).and_then(|s| s.shared_keys.clone())
    }

    /// Create a new scope, which keeps its data allocated.
    pub fn new_alloced(
        data: impl Into<Arc<[u8]>>,
        shared_keys: Option<Arc<SharedKeys>>,
    ) -> Option<Arc<Self>> {
        let scope_map = Self::scope_map();
        let mut scope_map = scope_map.write().ok()?;

        let strong_data = data.into();
        let data = Arc::downgrade(&strong_data);

        let start = strong_data.as_ptr() as usize;
        let end = strong_data.as_ptr() as usize + strong_data.len();

        let root = Self::root_or_null(&strong_data);

        let scope = Arc::new(Scope {
            shared_keys,
            weak_data: data,
            strong_data: Some(strong_data),
            root,
            registered: AtomicBool::new(true),
        });

        // TODO: Figure out how to protect against overlaps
        scope_map.insert(start..end, ScopeEntry(Arc::downgrade(&scope)));

        Some(scope)
    }

    /// Create a new Scope, which does not keep its data allocated.
    pub fn new_weak(data: Weak<[u8]>, shared_keys: Option<Arc<SharedKeys>>) -> Option<Arc<Self>> {
        let scope_map = Self::scope_map();
        let mut scope_map = scope_map.write().ok()?;

        let strong_data = data.upgrade()?;

        let start = strong_data.as_ptr() as usize;
        let end = strong_data.as_ptr() as usize + strong_data.len();

        let root = Self::root_or_null(&strong_data);

        let scope = Arc::new(Scope {
            shared_keys,
            weak_data: data,
            strong_data: None,
            root,
            registered: AtomicBool::new(true),
        });

        scope_map.insert(start..end, ScopeEntry(Arc::downgrade(&scope)));

        Some(scope)
    }

    /// Return a `ScopedValue` containing the root Value belonging to this Scope.
    pub fn root(&self) -> Option<ScopedValue> {
        self.data().and_then(|data| {
            if self.root.is_null() {
                None
            } else {
                Some(ScopedValue {
                    _data: data,
                    value: NonNull::from(unsafe { &*self.root }),
                })
            }
        })
    }

    pub fn data(&self) -> Option<Arc<[u8]>> {
        if let Some(alloced_data) = &self.strong_data {
            Some(alloced_data.clone())
        } else {
            self.weak_data.upgrade()
        }
    }

    fn scope_map() -> &'static RwLock<RangeMap<usize, ScopeEntry>> {
        SCOPE_MAP.get_or_init(|| RwLock::new(RangeMap::new()))
    }

    fn root_or_null(data: &[u8]) -> *const Value {
        Value::from_bytes(data).map_or_else(
            |_| ptr::slice_from_raw_parts(NonNull::<u8>::dangling().as_ptr(), 0) as *const Value,
            ptr::from_ref,
        )
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
    _data: Arc<[u8]>,
    value: NonNull<Value>,
}

impl ScopedValue {
    pub fn value(&self) -> &Value {
        unsafe { self.value.as_ref() }
    }
}

impl PartialEq for Scope {
    fn eq(&self, other: &Self) -> bool {
        let Some(self_data) = self.weak_data.upgrade() else {
            return false;
        };
        let Some(other_data) = other.weak_data.upgrade() else {
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
