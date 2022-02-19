use std::collections::HashMap;

use classfile_parser::field_info::FieldAccessFlags;
use rhojvm_base::{id::ClassId, util::MemorySize, ClassNames};

use crate::{
    gc::GcRef,
    rv::{RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    util::JavaString,
};

macro_rules! impl_instance_conv {
    ($variant_name:ident => $name:ty) => {
        impl From<$name> for Instance {
            fn from(v: $name) -> Instance {
                Instance::$variant_name(v)
            }
        }

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
#[derive(Debug)]
pub enum Instance {
    Class(ClassInstance),
    StaticClass(StaticClassInstance),
    PrimitiveArray(PrimitiveArrayInstance),
    ReferenceArray(ReferenceArrayInstance),
}
impl Instance {
    /// Note that this does not peek upwards (for class instances) into the static class
    /// for its fields.
    pub(crate) fn fields(&self) -> impl Iterator<Item = (&str, &Field)> {
        match self {
            Instance::Class(x) => x.fields.iter(),
            Instance::StaticClass(x) => x.fields.iter(),
            Instance::PrimitiveArray(x) => x.fields.iter(),
            Instance::ReferenceArray(x) => x.fields.iter(),
        }
    }
}
impl MemorySize for Instance {
    fn memory_size(&self) -> usize {
        // TODO: Our current memory size implementations don't include their sub-fields
        match self {
            Instance::Class(x) => x.memory_size(),
            Instance::StaticClass(x) => x.memory_size(),
            Instance::PrimitiveArray(x) => x.memory_size(),
            Instance::ReferenceArray(x) => x.memory_size(),
        }
    }
}

/// An instance of some class
#[derive(Debug, Clone)]
pub struct ClassInstance {
    /// The most specific Class that this is an instance of
    pub instanceof: ClassId,
    /// The static class instance of the class that this is an instance of
    pub static_ref: GcRef<StaticClassInstance>,
    /// Fields that it owns
    pub fields: Fields,
}
impl ClassInstance {
    pub fn new(
        instanceof: ClassId,
        static_ref: GcRef<StaticClassInstance>,
        fields: Fields,
    ) -> ClassInstance {
        ClassInstance {
            instanceof,
            static_ref,
            fields,
        }
    }
}
impl_instance_conv!(Class => ClassInstance);
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
impl StaticClassInstance {
    pub(crate) fn new(id: ClassId, fields: Fields) -> StaticClassInstance {
        StaticClassInstance { id, fields }
    }
}
impl_instance_conv!(StaticClass => StaticClassInstance);
impl MemorySize for StaticClassInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

// TODO: Specialized array instances for each kind of value?
#[derive(Debug, Clone)]
pub struct PrimitiveArrayInstance {
    pub instanceof: ClassId,
    pub element_type: RuntimeTypePrimitive,
    pub elements: Vec<RuntimeValuePrimitive>,
    // TODO: This only exists so that we can return an empty iterator over it that is the same type
    // as those returned in the impl iterator for instance. We should simply make an empty version.
    fields: Fields,
}
impl PrimitiveArrayInstance {
    pub(crate) fn new(
        instanceof: ClassId,
        element_type: RuntimeTypePrimitive,
        elements: Vec<RuntimeValuePrimitive>,
    ) -> PrimitiveArrayInstance {
        PrimitiveArrayInstance {
            instanceof,
            element_type,
            elements,
            fields: Fields::default(),
        }
    }

    #[must_use]
    pub fn len(&self) -> i32 {
        self.elements.len() as i32
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}
impl_instance_conv!(PrimitiveArray => PrimitiveArrayInstance);
impl MemorySize for PrimitiveArrayInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[derive(Debug, Clone)]
pub struct ReferenceArrayInstance {
    pub instanceof: ClassId,
    pub element_type: ClassId,
    pub elements: Vec<Option<GcRef<Instance>>>,
    // TODO: This only exists so that we can return an empty iterator over it that is the same type
    // as those returned in the impl iterator for instance. We should simply make an empty version.
    fields: Fields,
}
impl ReferenceArrayInstance {
    pub(crate) fn new(
        instanceof: ClassId,
        element_type: ClassId,
        elements: Vec<Option<GcRef<Instance>>>,
    ) -> ReferenceArrayInstance {
        ReferenceArrayInstance {
            instanceof,
            element_type,
            elements,
            fields: Fields::default(),
        }
    }

    #[must_use]
    pub fn len(&self) -> i32 {
        self.elements.len() as i32
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}
impl_instance_conv!(ReferenceArray => ReferenceArrayInstance);
impl MemorySize for ReferenceArrayInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[derive(Default, Debug, Clone)]
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

    pub fn insert(&mut self, name: impl Into<String>, field: Field) {
        self.fields.insert(name.into(), field);
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
    pub(crate) fn new(value: RuntimeValue, is_final: bool, access: FieldAccess) -> Field {
        Field {
            value,
            is_final,
            access,
        }
    }

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
impl FieldAccess {
    pub(crate) fn from_access_flags(flags: FieldAccessFlags) -> FieldAccess {
        if flags.contains(FieldAccessFlags::PRIVATE) {
            FieldAccess::Private
        } else if flags.contains(FieldAccessFlags::PROTECTED) {
            FieldAccess::Protected
        } else {
            FieldAccess::Public
        }
    }
}
