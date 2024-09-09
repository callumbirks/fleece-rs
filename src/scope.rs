use std::{
    ops::Deref,
    ptr::NonNull,
    sync::{Arc, Weak},
};

use crossbeam_utils::sync::ShardedLock;
use lazy_static::lazy_static;
use rangemap::RangeMap;

use crate::{alloced::AllocedValue, SharedKeys, Value};

#[derive(Debug)]
pub struct Scope {
    shared_keys: Option<Arc<SharedKeys>>,
    weak_data: Weak<[u8]>,
    strong_data: Option<Arc<[u8]>>,
    root: Option<NonNull<Value>>,
}

impl Scope {
    /// Find [`SharedKeys`] which are held by some [`Scope`] containing the given data.
    #[inline]
    #[must_use]
    pub fn find_shared_keys(containing_data: *const u8) -> Option<Arc<SharedKeys>> {
        Scope::containing(containing_data).and_then(|s| s.shared_keys.clone())
    }

    pub fn shared_keys(&self) -> Option<&Arc<SharedKeys>> {
        self.shared_keys.as_ref()
    }

    /// The data retained by this scope. Returns [`None`] if the data has been deallocated.
    pub fn data(&self) -> Option<Arc<[u8]>> {
        if let Some(strong_data) = &self.strong_data {
            Some(strong_data.clone())
        } else {
            self.weak_data.upgrade()
        }
    }

    /// The root [`Value`] contained in the data retained by this scope. Returns [`None`] if the data has been deallocated.
    pub fn root(&self) -> Option<AllocedValue> {
        self.data().and_then(|data| {
            self.root.map(|root| AllocedValue {
                buf: data,
                value: root.as_ptr(),
            })
        })
    }

    /// The range of memory that this scope retains. Returns [`None`] if the data has been deallocated.
    pub fn range(&self) -> Option<std::ops::Range<usize>> {
        self.data().map(|data| {
            let start = data.as_ptr() as usize;
            start..start + data.len()
        })
    }

    /// If the data in this scope is still being retained, release it. It will be dropped from the global retention mechanism,
    /// but the data will not be deallocated until all references to it are dropped.
    pub fn remove(&self) {
        let mut scope_map = SCOPE_MAP.write().unwrap();
        if let Some(range) = self.range() {
            scope_map.remove(range);
        }
    }

    /// Create a new scope which retains the data it is given ownership of, and optionally retains the given [`SharedKeys`], which should be relevant to the data.
    pub(crate) fn new(
        data: impl Into<Arc<[u8]>>,
        shared_keys: Option<Arc<SharedKeys>>,
    ) -> Arc<Self> {
        let mut scope_map = SCOPE_MAP.write().unwrap();
        let strong_data = data.into();
        let weak_data = Arc::downgrade(&strong_data);

        let start = strong_data.as_ptr() as usize;
        let end = start + strong_data.len();

        let root = Self::root_or_none(&strong_data);

        let scope = Arc::new(Scope {
            shared_keys,
            weak_data,
            strong_data: Some(strong_data),
            root,
        });

        scope_map.insert(start..end, ScopeEntry(Arc::downgrade(&scope)));
        scope
    }

    #[inline]
    fn root_or_none(data: &[u8]) -> Option<NonNull<Value>> {
        Value::from_bytes(data).map(NonNull::from).ok()
    }

    fn containing(data: *const u8) -> Option<Arc<Self>> {
        let scope_map = SCOPE_MAP.read().unwrap();
        let entry = scope_map.get(&(data as usize))?;
        entry.upgrade()
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

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

unsafe impl Send for Scope {}
unsafe impl Sync for Scope {}

lazy_static! {
    static ref SCOPE_MAP: ShardedLock<RangeMap<usize, ScopeEntry>> =
        ShardedLock::new(RangeMap::new());
}
