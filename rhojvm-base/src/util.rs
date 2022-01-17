use std::path::PathBuf;

use classfile_parser::constant_pool::{ConstantPoolIndex, ConstantPoolIndexRaw};

// We can't really have a generic version for packages and classes because we can't
// distinguish between a class path and a package path
/// Convert the access path for a class into a relative path
pub(crate) fn class_path_slice_to_relative_path<T: AsRef<str>>(class_path: &[T]) -> PathBuf {
    let mut path = PathBuf::with_capacity(class_path.len());
    for path_part in class_path.iter() {
        path.push(path_part.as_ref());
    }

    path.set_extension("class");

    path
}
pub(crate) fn class_path_iter_to_relative_path<'a>(
    class_path: impl Iterator<Item = &'a str> + Clone,
) -> PathBuf {
    let count = class_path.clone().count();
    let mut path = PathBuf::with_capacity(count);
    for path_part in class_path {
        path.push(path_part);
    }

    path.set_extension("class");

    path
}

/// Note: This will work fine for path to a class as well
#[must_use]
pub fn access_path_iter(package: &str) -> impl DoubleEndedIterator<Item = &str> + Clone {
    package.split('/')
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

/// Memory size but for types that are always less than [`u16::MAX`]
pub trait MemorySizeU16 {
    fn memory_size_u16(&self) -> u16;
}
pub trait StaticMemorySizeU16 {
    const MEMORY_SIZE_U16: u16;
}
impl<T: StaticMemorySizeU16> MemorySizeU16 for T {
    fn memory_size_u16(&self) -> u16 {
        T::MEMORY_SIZE_U16
    }
}
impl<T: StaticMemorySizeU16> StaticMemorySize for T {
    const MEMORY_SIZE: usize = T::MEMORY_SIZE_U16 as usize;
}
impl StaticMemorySizeU16 for bool {
    const MEMORY_SIZE_U16: u16 = 1;
}
impl StaticMemorySizeU16 for u8 {
    const MEMORY_SIZE_U16: u16 = 1;
}
impl StaticMemorySizeU16 for i8 {
    const MEMORY_SIZE_U16: u16 = 1;
}
impl StaticMemorySizeU16 for u16 {
    const MEMORY_SIZE_U16: u16 = 2;
}
impl StaticMemorySizeU16 for i16 {
    const MEMORY_SIZE_U16: u16 = 2;
}
impl StaticMemorySizeU16 for u32 {
    const MEMORY_SIZE_U16: u16 = 4;
}
impl StaticMemorySizeU16 for i32 {
    const MEMORY_SIZE_U16: u16 = 4;
}
impl StaticMemorySizeU16 for u64 {
    const MEMORY_SIZE_U16: u16 = 8;
}
impl StaticMemorySizeU16 for i64 {
    const MEMORY_SIZE_U16: u16 = 8;
}
impl<T> StaticMemorySizeU16 for ConstantPoolIndexRaw<T> {
    const MEMORY_SIZE_U16: u16 = u16::MEMORY_SIZE_U16;
}
impl<T> StaticMemorySizeU16 for ConstantPoolIndex<T> {
    const MEMORY_SIZE_U16: u16 = u16::MEMORY_SIZE_U16;
}

// We wrap this because the alternative hasher is not generic
// and Rust doesn't allow unused generics.
// But this allows us to have that.
// TODO: Is there a better way to implement this?
pub(crate) trait HashWrapperTrait<T> {
    type HashMapHasher;

    fn identity(v: T) -> T {
        v
    }
}
pub(crate) struct HashWrapper;
impl<T> HashWrapperTrait<T> for HashWrapper {
    #[cfg(feature = "implementation-cheaper-map-hashing")]
    type HashMapHasher = nohash_hasher::BuildNoHashHasher<T>;
    #[cfg(not(feature = "implementation-cheaper-map-hashing"))]
    type HashMapHasher = std::collections::hash_map::RandomState;
}

/// A map that only publicly exposes basic methods
/// Note that you should prefix it with `typical` if your id is not a simple integer.
/// Private macro.
#[macro_export]
macro_rules! __make_map {
    (typical $v:vis $name:ident < $key:ty, $val:ty > $(; $($tag:ident),*)?) => {
        #[derive(Default, Clone)]
        $v struct $name {
            map: std::collections::HashMap<$key, $val>,
        }
        #[allow(dead_code)]
        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self {
                    map: std::collections::HashMap::new(),
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

            /// Takes the value out
            /// NOTE: This should really only be used if you know what you're doing.
            pub fn remove(&mut self, key: &$key) -> Option<$val> {
                self.map.remove(key)
            }
        }

        $(
            $(
                __make_map!(I $tag $name < $key, $val >);
            )*
        )?
    };
    ($v:vis $name:ident < $key:ty, $val:ty > $(; $($tag:ident),*)?) => {
        #[derive(Default, Clone)]
        $v struct $name {
            map: std::collections::HashMap<$key, $val, <$crate::util::HashWrapper as $crate::util::HashWrapperTrait<$key>>::HashMapHasher>,
        }
        #[allow(dead_code)]
        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self {
                    map: std::collections::HashMap::with_hasher(<$crate::util::HashWrapper as $crate::util::HashWrapperTrait<$key>>::HashMapHasher::default()),
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

            /// Takes the value out
            /// NOTE: This should really only be used if you know what you're doing.
            pub fn remove(&mut self, key: &$key) -> Option<$val> {
                self.map.remove(key)
            }
        }

        $(
            $(
                __make_map!(I $tag $name < $key, $val >);
            )*
        )?

    };
    (I access $name:ident < $key:ty, $val:ty >) => {
        #[allow(dead_code)]
        impl $name {
            #[must_use]
            pub fn get(&self, key: &$key) -> Option<&$val> {
                self.map.get(key)
            }

            /// You MUST NOT swap an incorrect instance into the position.
            /// Ex: Do not construct a class and then swap it with this one,
            /// as that leads to an invalid mapping of class-id to class.
            #[must_use]
            pub fn get_mut(&mut self, key: &$key) -> Option<&mut $val> {
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
                    panic!();
                }
            }

            #[must_use]
            pub fn iter(&self) -> std::collections::hash_map::Iter<$key, $val> {
                self.map.iter()
            }
        }
    }
}
