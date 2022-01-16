use std::hash::Hasher;

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
pub type MethodIndex = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// Note: the returned hasher must consider `write_bytes` to be the equivalent
/// to writing each byte one time
/// so that writing strings behaves properly
pub(crate) fn make_hasher() -> impl Hasher {
    // TODO: Should we use 128 bit output version?
    // We explicitly specify the keys so that it will be stable
    siphasher::sip::SipHasher::new_with_keys(0, 0)
}

pub(crate) fn make_hasher1238() -> impl siphasher::sip128::Hasher128 + Hasher {
    siphasher::sip128::SipHasher::new_with_keys(0, 0)
}

#[must_use]
pub(crate) fn hash_access_path(path: &str) -> HashId {
    let mut state = make_hasher();
    state.write(path.as_bytes());
    state.write_u8(0xff);
    state.finish()
}

#[must_use]
pub(crate) fn hash_access_path_slice<T: AsRef<str>>(path: &[T]) -> HashId {
    let mut state = make_hasher();
    if path.get(0).map_or(false, |x| is_array_class(x.as_ref())) && path.len() > 1 {
        tracing::warn!(
            "hash_access_path_slice received slice of array type with more than one entry"
        );
        panic!("");
    }
    for entry in itertools::intersperse(path.iter().map(AsRef::as_ref), "/") {
        state.write(entry.as_bytes());
    }
    state.write_u8(0xff);
    state.finish()
}

#[must_use]
/// NOTE: Currently all hashing should go through this
/// because hashing a string is not the same as hashing the individual
/// characters
/// This method does handle array classes properly.
pub(crate) fn hash_access_path_iter<'a>(
    path: impl Iterator<Item = &'a str> + Clone,
    is_single_str: bool,
) -> HashId {
    let count = path.clone().count();
    let mut state = make_hasher();

    // Check for arrays since they shouldn't be hashed in the same manner
    let mut path = path.peekable();
    if path.peek().map_or(false, |x| is_array_class(x)) {
        if count > 1 && !is_single_str {
            tracing::warn!(
                "hash_access_path_iter received iterator of array type with more than one entry"
            );
            panic!("");
        }
        hash_from_iter(&mut state, path);
    } else {
        // TODO: use hash_from_iter once intersperse is stabilized

        let path = itertools::intersperse(path, "/");
        hash_from_iter(&mut state, path);
    }

    state.finish()
}

/// Hashes each entry in the iterator as if it was one contiguous string
/// This helps avoid the problem that doing:
/// `"hello".hash(&mut hasher);`
/// is not the same as
/// `"hell".hash(&mut hasher); "o".hash(&mut hasher);`
/// Looking at the hash implementation for str, we can see that it hashes the bytes of the string
/// and then 0xff.
/// This means that you can't simply hash the str in two parts to get the same result, and since it
/// is through bytes, you can't use char (since that is treated as a u32 rather than the
/// smaller utf8 size that strs hold).
/// This function _requires_ that the hasher treat writing the same _bytes_ as equivalent, no matter
/// the order.
fn hash_from_iter<'a>(state: &mut impl Hasher, data: impl Iterator<Item = &'a str>) {
    for entry in data {
        state.write(entry.as_bytes());
    }
    // Hash 0xff so that it acts like a normal long str
    state.write_u8(0xff);
}

pub(crate) fn is_array_class(first: &str) -> bool {
    first.starts_with('[')
}
