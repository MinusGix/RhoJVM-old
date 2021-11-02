use std::path::PathBuf;

use classfile_parser::constant_pool::{ConstantPoolIndex, ConstantPoolIndexRaw};

// We can't really have a generic version for packages and classes because we can't
// distinguish between a class path and a package path
/// Convert the access path for a class into a relative path
pub(crate) fn class_path_slice_to_relative_path<T: AsRef<str>>(class_path: &[T]) -> PathBuf {
    let mut path = PathBuf::new();
    for (i, path_part) in class_path.iter().enumerate() {
        if (i + 1) < class_path.len() {
            path.push(path_part.as_ref());
        } else {
            path.push(format!("{}.class", path_part.as_ref()));
        }
    }

    path
}
pub(crate) fn class_path_iter_to_relative_path<'a>(
    class_path: impl Iterator<Item = &'a str> + Clone,
) -> PathBuf {
    let mut path = PathBuf::new();
    let count = class_path.clone().count();
    for (i, path_part) in class_path.enumerate() {
        if (i + 1) < count {
            path.push(path_part);
        } else {
            path.push(format!("{}.class", path_part));
        }
    }

    path
}

/// Note: This will work fine for path to a class as well
#[must_use]
pub fn access_path_iter(package: &str) -> impl DoubleEndedIterator<Item = &str> + Clone {
    // TODO: Currently we allow both . and / but it would be nice to stabilize on one.
    package.split(|x| x == '.' || x == '/')
}

pub trait MemorySize {
    /// Note: this only applies to the *direct* memory size
    fn memory_size(&self) -> usize;
}
pub trait StaticMemorySize {
    const MEMORY_SIZE: usize;
}
impl<T: StaticMemorySize> MemorySize for T {
    fn memory_size(&self) -> usize {
        T::MEMORY_SIZE
    }
}
impl StaticMemorySize for bool {
    const MEMORY_SIZE: usize = 1;
}
impl StaticMemorySize for u8 {
    const MEMORY_SIZE: usize = 1;
}
impl StaticMemorySize for i8 {
    const MEMORY_SIZE: usize = 1;
}
impl StaticMemorySize for u16 {
    const MEMORY_SIZE: usize = 2;
}
impl StaticMemorySize for i16 {
    const MEMORY_SIZE: usize = 2;
}
impl StaticMemorySize for u32 {
    const MEMORY_SIZE: usize = 4;
}
impl StaticMemorySize for i32 {
    const MEMORY_SIZE: usize = 4;
}
impl StaticMemorySize for u64 {
    const MEMORY_SIZE: usize = 8;
}
impl StaticMemorySize for i64 {
    const MEMORY_SIZE: usize = 8;
}
impl<T> StaticMemorySize for ConstantPoolIndexRaw<T> {
    const MEMORY_SIZE: usize = u16::MEMORY_SIZE;
}
impl<T> StaticMemorySize for ConstantPoolIndex<T> {
    const MEMORY_SIZE: usize = u16::MEMORY_SIZE;
}

/// A map that only publicly exposes immutable methods
/// Private macro.
#[macro_export]
macro_rules! __make_map {
    ($v:vis $name:ident < $key:ty, $val:ty >) => {
        $v struct $name {
            map: std::collections::BTreeMap<$key, $val>,
        }
        #[allow(dead_code)]
        impl $name {
            #[must_use]
            pub(crate) fn new() -> Self {
                Self {
                    map: BTreeMap::new(),
                }
            }

            #[must_use]
            pub fn len(&self) -> usize {
                self.map.len()
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.map.is_empty()
            }

            #[must_use]
            pub fn contains_key(&self, key: &$key) -> bool {
                self.map.contains_key(key)
            }

            #[must_use]
            pub fn get(&self, key: &$key) -> Option<&$val> {
                self.map.get(key)
            }

            #[must_use]
            pub(crate) fn get_mut(&mut self, key: &$key) -> Option<&mut $val> {
                self.map.get_mut(key)
            }

            /// Panics in debug mode if the key already exists
            /// This helps find accidental multi-sets, since the maps should
            /// not do that.
            pub(crate) fn set_at(&mut self, key: $key, val: $val) {
                let key2 = key.clone();
                let e = self.map.insert(key, val);
                if e.is_some() {
                    tracing::warn!("Duplicate Setting for map '{}' with {:?}", stringify!($name), key2);
                }
            }

            #[must_use]
            pub fn iter(&self) -> std::collections::btree_map::Iter<$key, $val> {
                self.map.iter()
            }
        }

    };
}
