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

#[must_use]
pub fn access_path_iter_bytes(package: &[u8]) -> impl DoubleEndedIterator<Item = &[u8]> + Clone {
    package.split(|x| *x == b'/')
}

/// Return with only the initial parts
#[must_use]
pub fn access_path_initial_part(package: &[u8]) -> Option<&[u8]> {
    let last_index = package
        .iter()
        .enumerate()
        .rev()
        .find(|x| *x.1 == b'/')
        .map(|x| x.0);

    if let Some(last_index) = last_index {
        Some(&package[..last_index])
    } else {
        // If there is no last index then this is likely a lone class name
        // and so there is no package
        None
    }
}

pub(crate) fn format_class_as_object_desc(class_name: &[u8]) -> Vec<u8> {
    let mut res = Vec::with_capacity(2 + class_name.len());
    res.push(b'L');
    res.extend_from_slice(class_name);
    res.push(b';');
    res
}

pub trait MemorySize {
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

impl MemorySize for String {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<String>() + self.as_bytes().len() * u8::MEMORY_SIZE
    }
}
impl<T: MemorySize> MemorySize for Vec<T> {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Vec<T>>() + (self.iter().fold(0, |acc, x| acc + x.memory_size()))
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
