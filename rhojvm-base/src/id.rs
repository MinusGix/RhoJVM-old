use std::hash::{Hash, Hasher};

use crate::util;

/// We make use of hashes of paths/names for a lot of these types as that makes them deterministic
/// across runs, which is a nice a property to have in general.
/// It would allow storing of precomputed data
pub type HashId = u64;
pub type ClassFileId = HashId;
pub type ClassId = HashId;
pub type GeneralClassId = HashId;

pub type PackageId = HashId;

/// This is an index into the methods
/// This is not meaningful without a class
pub type MethodIndex = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

pub(crate) fn make_hasher() -> impl Hasher {
    // TODO: Should we use 128 bit output version?
    // We explicitly specify the keys so that it will be stable
    siphasher::sip::SipHasher::new_with_keys(0, 0)
}

#[must_use]
pub(crate) fn hash_access_path(path: &str) -> HashId {
    if is_array_class(path) {
        hash_access_path_iter([path].into_iter())
    } else {
        hash_access_path_iter(util::access_path_iter(path))
    }
}

#[must_use]
pub(crate) fn hash_access_path_slice<T: AsRef<str>>(path: &[T]) -> HashId {
    hash_access_path_iter(path.iter().map(AsRef::as_ref))
}

#[must_use]
// NOTE: Currently all hashing should go through this
// because we are unsure if there is any assurance that hashing
// "java/lang/Object" is equivalent to hashing the individual
// "java" '/' "lang" '/' "Object"
// and so this function simply does it the latter way.
// This method does handle array classes properly.
pub(crate) fn hash_access_path_iter<'a>(path: impl Iterator<Item = &'a str> + Clone) -> HashId {
    let count = path.clone().count();
    let mut state = make_hasher();

    // Check for arrays since they shouldn't be hashed in the same manner
    let mut path = path.peekable();
    if path.peek().map_or(false, |x| is_array_class(x)) {
        for (i, part) in path.enumerate() {
            if i > 0 {
                tracing::warn!("hash_access_path_iter received iterator of array type with more than one entry");
                panic!("");
            }
            part.hash(&mut state);
        }
    } else {
        for (i, part) in path.enumerate() {
            part.hash(&mut state);
            if i + 1 != count {
                '/'.hash(&mut state);
            }
        }
    }

    state.finish()
}

pub(crate) fn is_array_class(first: &str) -> bool {
    first.starts_with('[')
}
