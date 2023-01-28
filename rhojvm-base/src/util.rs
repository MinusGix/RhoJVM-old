use classfile_parser::constant_pool::{ConstantPoolIndex, ConstantPoolIndexRaw};

#[derive(Clone, Eq, PartialEq)]
pub struct Cesu8String(pub Vec<u8>);
impl std::fmt::Display for Cesu8String {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text =
            cesu8::from_java_cesu8(&self.0).unwrap_or_else(|_| String::from_utf8_lossy(&self.0));
        f.write_fmt(format_args!("{}", text))
    }
}
impl std::fmt::Debug for Cesu8String {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text =
            cesu8::from_java_cesu8(&self.0).unwrap_or_else(|_| String::from_utf8_lossy(&self.0));
        f.write_fmt(format_args!("\"{}\"", text))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Cesu8Str<'a>(pub &'a [u8]);
impl<'a> std::fmt::Display for Cesu8Str<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text =
            cesu8::from_java_cesu8(self.0).unwrap_or_else(|_| String::from_utf8_lossy(self.0));
        f.write_fmt(format_args!("{}", text))
    }
}
impl<'a> std::fmt::Debug for Cesu8Str<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text =
            cesu8::from_java_cesu8(self.0).unwrap_or_else(|_| String::from_utf8_lossy(self.0));
        f.write_fmt(format_args!("\"{}\"", text))
    }
}

/// Tries converting cesu8-java-style strings into Rust's utf8 strings
/// This tries to avoid allocating but may not be able to avoid it
#[must_use]
pub fn convert_classfile_text(bytes: &[u8]) -> std::borrow::Cow<str> {
    cesu8::from_java_cesu8(bytes).unwrap_or_else(|_| String::from_utf8_lossy(bytes))
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
