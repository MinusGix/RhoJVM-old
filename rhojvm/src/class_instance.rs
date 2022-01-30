use std::collections::HashMap;

use rhojvm_base::{id::ClassId, util::MemorySize};

use crate::{rv::RuntimeValue, util::JavaString};

/// An instance of a class, made generic over several common variants
pub enum Instance {
    Class(ClassInstance),
    StaticClass(StaticClassInstance),
    String(StringInstance),
}
impl Instance {
    /// Note that this does not peek upwards (for class instances) into the static class
    /// for its fields.
    pub(crate) fn fields(&self) -> impl Iterator<Item = (&str, &Field)> {
        match self {
            Instance::Class(x) => x.fields.iter(),
            Instance::StaticClass(x) => x.fields.iter(),
            // TODO: If we fake some of the fields a string holds then we have to fake them here too
            Instance::String(x) => x.fields.iter(),
        }
    }
}
impl MemorySize for Instance {
    fn memory_size(&self) -> usize {
        // TODO: This could be better..
        match self {
            Instance::Class(x) => x.memory_size(),
            Instance::StaticClass(x) => x.memory_size(),
            Instance::String(x) => x.memory_size(),
        }
    }
}

/// An instance of some class
#[derive(Debug, Clone)]
pub struct ClassInstance {
    /// The most specific Class that this is an instance of
    pub instanceof: ClassId,
    /// Fields that it owns
    pub fields: Fields,
}
impl MemorySize for ClassInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/// The class that is the static class, which may have its own fields and the like on it
#[derive(Debug, Clone)]
pub struct StaticClassInstance {
    /// Its own id
    pub id: ClassId,
    /// Static fields
    pub fields: Fields,
}
impl MemorySize for StaticClassInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[derive(Debug, Clone)]
pub struct StringInstance {
    pub value: JavaString,
    // For now, we use the fields just like typical classes, but we could apply
    // an optimization to skip storing anything within these fields where we want
    // to handle it ourselves, such as the length and data
    fields: Fields,
}
impl MemorySize for StringInstance {
    fn memory_size(&self) -> usize {
        self.value.memory_size()
    }
}

#[derive(Debug, Clone)]
pub struct Fields {
    fields: HashMap<String, Field>,
}
impl Fields {
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Field)> {
        self.fields.iter().map(|x| (x.0.as_ref(), x.1))
    }
}

/// A field with some value
/// This does not keep track of the name
/// It also does not keep track of whether it is static because that is decided by the outside
#[derive(Debug, Clone)]
pub struct Field {
    value: RuntimeValue,
    /// Whether it is a final value or not, so it cannot change after initialization
    is_final: bool,
    access: FieldAccess,
}
impl Field {
    #[must_use]
    pub fn value(&self) -> RuntimeValue {
        self.value
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FieldAccess {
    Public,
    Protected,
    Private,
}
