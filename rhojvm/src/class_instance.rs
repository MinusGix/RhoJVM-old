use std::collections::HashMap;

use rhojvm_base::{id::ClassId, util::MemorySize};

use crate::{
    rv::{RuntimeType, RuntimeValue},
    util::JavaString,
};

macro_rules! try_from_instance {
    ($variant_name:ident => $name:ty) => {
        impl<'a> TryFrom<&'a Instance> for &'a $name {
            type Error = ();
            fn try_from(i: &'a Instance) -> Result<&'a $name, ()> {
                match i {
                    Instance::$variant_name(x) => Ok(x),
                    _ => Err(()),
                }
            }
        }

        impl<'a> TryFrom<&'a mut Instance> for &'a mut $name {
            type Error = ();
            fn try_from(i: &'a mut Instance) -> Result<&'a mut $name, ()> {
                match i {
                    Instance::$variant_name(x) => Ok(x),
                    _ => Err(()),
                }
            }
        }
    };
}

/// An instance of a class, made generic over several common variants
pub enum Instance {
    Class(ClassInstance),
    StaticClass(StaticClassInstance),
    Array(ArrayInstance),
    String(StringInstance),
}
impl Instance {
    /// Note that this does not peek upwards (for class instances) into the static class
    /// for its fields.
    pub(crate) fn fields(&self) -> impl Iterator<Item = (&str, &Field)> {
        match self {
            Instance::Class(x) => x.fields.iter(),
            Instance::StaticClass(x) => x.fields.iter(),
            Instance::Array(x) => x.fields.iter(),
            // TODO: If we fake some of the fields a string holds then we have to fake them here too
            Instance::String(x) => x.fields.iter(),
        }
    }
}
impl MemorySize for Instance {
    fn memory_size(&self) -> usize {
        // TODO: Our current memory size implementations don't include their sub-fields
        match self {
            Instance::Class(x) => x.memory_size(),
            Instance::StaticClass(x) => x.memory_size(),
            Instance::Array(x) => x.memory_size(),
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
try_from_instance!(Class => ClassInstance);
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
try_from_instance!(StaticClass => StaticClassInstance);
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
try_from_instance!(String => StringInstance);
impl MemorySize for StringInstance {
    fn memory_size(&self) -> usize {
        self.value.memory_size()
    }
}

// TODO: Specialized array instances for each kind of value?
#[derive(Debug, Clone)]
pub struct ArrayInstance {
    pub element_type: RuntimeType,
    pub elements: Vec<RuntimeValue>,
    // TODO: This only exists so that we can return an empty iterator over it that is the same type
    // as those returned in the impl iterator for instance. We should simply make an empty version.
    fields: Fields,
}
try_from_instance!(Array => ArrayInstance);
impl MemorySize for ArrayInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[derive(Debug, Clone)]
pub struct Fields {
    fields: HashMap<String, Field>,
}
impl Fields {
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Field> {
        self.fields.get(name)
    }

    #[must_use]
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Field> {
        self.fields.get_mut(name)
    }

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
