use std::hash::{Hash, Hasher};

#[derive(Debug, Copy, Clone)]
pub struct ClassId(u32);
impl ClassId {
    pub(crate) fn new_unchecked(id: u32) -> ClassId {
        ClassId(id)
    }

    pub(crate) fn get(self) -> u32 {
        self.0
    }
}

// This only really holds true if they're from the same [`ClassNames`] instance
impl PartialEq for ClassId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for ClassId {}
impl Hash for ClassId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.0)
    }
}
#[cfg(feature = "implementation-cheaper-map-hashing")]
impl nohash_hasher::IsEnabled for ClassId {}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PackageId(u32);
impl PackageId {
    pub(crate) fn new_unchecked(id: u32) -> PackageId {
        PackageId(id)
    }
}

/// This is an index into the methods
/// This is not meaningful without a class
pub type MethodIndex = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MethodId {
    class_id: ClassId,
    method_index: MethodIndex,
}
impl MethodId {
    #[must_use]
    pub fn unchecked_compose(class_id: ClassId, method_index: MethodIndex) -> Self {
        Self {
            class_id,
            method_index,
        }
    }

    #[must_use]
    pub fn decompose(self) -> (ClassId, MethodIndex) {
        (self.class_id, self.method_index)
    }
}

pub(crate) fn is_array_class(first: &str) -> bool {
    first.starts_with('[')
}

pub(crate) fn is_array_class_bytes(first: &[u8]) -> bool {
    first.starts_with(&[b'['])
}
