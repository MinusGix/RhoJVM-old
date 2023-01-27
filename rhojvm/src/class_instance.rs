use std::{collections::HashMap, hash::Hash, thread::ThreadId};

use classfile_parser::field_info::FieldAccessFlags;
use either::Either;
use rhojvm_base::{
    id::{ClassId, ExactMethodId},
    util::MemorySize,
};

use crate::{
    gc::{GcRef, GcValueMarker},
    rv::{RuntimeType, RuntimeTypePrimitive, RuntimeTypeVoid, RuntimeValue, RuntimeValuePrimitive},
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
macro_rules! impl_reference_instance_conv {
    ($variant_name:ident => $name:ty) => {
        impl From<$name> for Instance {
            fn from(v: $name) -> Instance {
                Instance::Reference(ReferenceInstance::from(v))
            }
        }
        impl From<$name> for ReferenceInstance {
            fn from(v: $name) -> ReferenceInstance {
                ReferenceInstance::$variant_name(v)
            }
        }

        impl<'a> TryFrom<&'a Instance> for &'a $name {
            type Error = ();
            fn try_from(i: &'a Instance) -> Result<&'a $name, ()> {
                match i {
                    Instance::Reference(x) => <&'a $name>::try_from(x),
                    _ => Err(()),
                }
            }
        }

        impl<'a> TryFrom<&'a ReferenceInstance> for &'a $name {
            type Error = ();
            fn try_from(i: &'a ReferenceInstance) -> Result<&'a $name, ()> {
                match i {
                    ReferenceInstance::$variant_name(x) => Ok(x),
                    _ => Err(()),
                }
            }
        }

        impl<'a> TryFrom<&'a mut Instance> for &'a mut $name {
            type Error = ();
            fn try_from(i: &'a mut Instance) -> Result<&'a mut $name, ()> {
                match i {
                    Instance::Reference(x) => <&'a mut $name>::try_from(x),
                    _ => Err(()),
                }
            }
        }

        impl<'a> TryFrom<&'a mut ReferenceInstance> for &'a mut $name {
            type Error = ();
            fn try_from(i: &'a mut ReferenceInstance) -> Result<&'a mut $name, ()> {
                match i {
                    ReferenceInstance::$variant_name(x) => Ok(x),
                    _ => Err(()),
                }
            }
        }
    };
}

/// An instance of a class, made generic over several common variants
#[derive(Clone, Debug)]
pub enum Instance {
    StaticClass(StaticClassInstance),
    Reference(ReferenceInstance),
}
impl Instance {
    /// Note that this does not peek upwards (for class instances) into the static class
    /// for its fields.
    pub(crate) fn fields(
        &self,
    ) -> Either<impl Iterator<Item = (FieldId, &Field)>, impl Iterator<Item = (FieldId, &Field)>>
    {
        match self {
            Instance::StaticClass(x) => Either::Left(x.fields.iter()),
            Instance::Reference(x) => Either::Right(x.fields()),
        }
    }
}
impl MemorySize for Instance {
    fn memory_size(&self) -> usize {
        // TODO: Our current memory size implementations don't include their sub-fields
        match self {
            Instance::StaticClass(x) => x.memory_size(),
            Instance::Reference(x) => x.memory_size(),
        }
    }
}
impl GcValueMarker for Instance {}

#[derive(Clone, Debug)]
pub enum ReferenceInstance {
    Class(ClassInstance),
    /// An instance of java/lang/Class
    StaticForm(StaticFormInstance),
    /// An instance of java/lang/Thread
    Thread(ThreadInstance),
    /// An instance of java/lang/invoke/MethodHandle
    MethodHandle(MethodHandleInstance),
    MethodHandleInfo(MethodHandleInfoInstance),
    PrimitiveArray(PrimitiveArrayInstance),
    ReferenceArray(ReferenceArrayInstance),
}
impl ReferenceInstance {
    /// Note that this does not peek upwards into the static class for its fields
    pub(crate) fn fields(&self) -> impl Iterator<Item = (FieldId, &Field)> {
        match self {
            ReferenceInstance::Class(x) => x.fields.iter(),
            ReferenceInstance::StaticForm(x) => x.inner.fields.iter(),
            ReferenceInstance::Thread(x) => x.inner.fields.iter(),
            ReferenceInstance::MethodHandle(x) => x.inner.fields.iter(),
            ReferenceInstance::MethodHandleInfo(x) => x.inner.fields.iter(),
            ReferenceInstance::PrimitiveArray(x) => x.fields.iter(),
            ReferenceInstance::ReferenceArray(x) => x.fields.iter(),
        }
    }

    pub(crate) fn instanceof(&self) -> ClassId {
        match self {
            ReferenceInstance::Class(x) => x.instanceof,
            ReferenceInstance::StaticForm(x) => x.inner.instanceof,
            ReferenceInstance::Thread(x) => x.inner.instanceof,
            ReferenceInstance::MethodHandle(x) => x.inner.instanceof,
            ReferenceInstance::MethodHandleInfo(x) => x.inner.instanceof,
            ReferenceInstance::PrimitiveArray(x) => x.instanceof,
            ReferenceInstance::ReferenceArray(x) => x.instanceof,
        }
    }

    /// Get the fields for Class instances
    /// This includes `ClassInstance` and `StaticFormInstance`
    pub(crate) fn get_class_fields(&self) -> Option<&Fields> {
        match self {
            ReferenceInstance::Class(x) => Some(&x.fields),
            ReferenceInstance::StaticForm(x) => Some(&x.inner.fields),
            ReferenceInstance::Thread(x) => Some(&x.inner.fields),
            ReferenceInstance::MethodHandle(x) => Some(&x.inner.fields),
            ReferenceInstance::MethodHandleInfo(x) => Some(&x.inner.fields),
            ReferenceInstance::PrimitiveArray(_) | ReferenceInstance::ReferenceArray(_) => None,
        }
    }

    /// Get the fields for Class instances
    /// This includes `ClassInstance` and `StaticFormInstance`
    pub(crate) fn get_class_fields_mut(&mut self) -> Option<&mut Fields> {
        match self {
            ReferenceInstance::Class(x) => Some(&mut x.fields),
            ReferenceInstance::StaticForm(x) => Some(&mut x.inner.fields),
            ReferenceInstance::Thread(x) => Some(&mut x.inner.fields),
            ReferenceInstance::MethodHandle(x) => Some(&mut x.inner.fields),
            ReferenceInstance::MethodHandleInfo(x) => Some(&mut x.inner.fields),
            ReferenceInstance::PrimitiveArray(_) | ReferenceInstance::ReferenceArray(_) => None,
        }
    }
}
impl MemorySize for ReferenceInstance {
    fn memory_size(&self) -> usize {
        match self {
            ReferenceInstance::Class(x) => x.memory_size(),
            ReferenceInstance::StaticForm(x) => x.memory_size(),
            ReferenceInstance::Thread(x) => x.memory_size(),
            ReferenceInstance::MethodHandle(x) => x.memory_size(),
            ReferenceInstance::MethodHandleInfo(x) => x.memory_size(),
            ReferenceInstance::PrimitiveArray(x) => x.memory_size(),
            ReferenceInstance::ReferenceArray(x) => x.memory_size(),
        }
    }
}
impl GcValueMarker for ReferenceInstance {}
impl From<ReferenceInstance> for Instance {
    fn from(x: ReferenceInstance) -> Self {
        Instance::Reference(x)
    }
}
impl TryFrom<Instance> for ReferenceInstance {
    type Error = ();

    fn try_from(value: Instance) -> Result<Self, Self::Error> {
        if let Instance::Reference(x) = value {
            Ok(x)
        } else {
            Err(())
        }
    }
}
impl<'a> TryFrom<&'a Instance> for &'a ReferenceInstance {
    type Error = ();

    fn try_from(value: &'a Instance) -> Result<Self, Self::Error> {
        if let Instance::Reference(x) = value {
            Ok(x)
        } else {
            Err(())
        }
    }
}
impl<'a> TryFrom<&'a mut Instance> for &'a mut ReferenceInstance {
    type Error = ();

    fn try_from(value: &'a mut Instance) -> Result<Self, Self::Error> {
        if let Instance::Reference(x) = value {
            Ok(x)
        } else {
            Err(())
        }
    }
}

/// A special cased structure for `Class<T>`
#[derive(Debug, Clone)]
pub struct StaticFormInstance {
    pub(crate) inner: ClassInstance,
    /// The T in Class<T>
    pub(crate) of: RuntimeTypeVoid<ClassId>,
    // The T in Class<T>
    // pub(crate) of_ref: Option<GcRef<StaticClassInstance>>,
}
impl StaticFormInstance {
    #[must_use]
    pub(crate) fn new(
        inner_instance: ClassInstance,
        of: impl Into<RuntimeTypeVoid<ClassId>>,
        _of_ref: Option<GcRef<StaticClassInstance>>,
    ) -> StaticFormInstance {
        StaticFormInstance {
            inner: inner_instance,
            of: of.into(),
            // of,
        }
    }
}
impl_reference_instance_conv!(StaticForm => StaticFormInstance);
impl MemorySize for StaticFormInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl GcValueMarker for StaticFormInstance {}

#[derive(Debug, Clone)]
pub struct ThreadInstance {
    pub(crate) inner: ClassInstance,
    pub thread_id: Option<ThreadId>,
}
impl ThreadInstance {
    #[must_use]
    pub fn new(inner_instance: ClassInstance, thread_id: Option<ThreadId>) -> ThreadInstance {
        ThreadInstance {
            inner: inner_instance,
            thread_id,
        }
    }

    /// Insert the `thread_id` into the structure, not bothering to check if it already exists
    pub(crate) fn fill_thread_id_unchecked(&mut self, thread_id: ThreadId) {
        self.thread_id = Some(thread_id);
    }
}
impl_reference_instance_conv!(Thread => ThreadInstance);
impl MemorySize for ThreadInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl GcValueMarker for ThreadInstance {}

#[derive(Debug, Clone)]
pub enum MethodHandleType {
    Constant {
        value: Option<GcRef<ReferenceInstance>>,
        /// The type that the method handle says it returns
        return_ty: RuntimeType<ClassId>,
    },
    InvokeStatic(ExactMethodId),
}
impl MethodHandleType {
    pub fn direct_kind(&self) -> Option<u8> {
        Some(match self {
            MethodHandleType::Constant { .. } => return None,
            MethodHandleType::InvokeStatic(_) => 6,
        })
    }

    pub fn is_direct(&self) -> bool {
        match self {
            MethodHandleType::Constant { .. } => false,
            MethodHandleType::InvokeStatic(_) => true,
        }
    }
}
/// An instance of `rho/invoke/MainMethodHandle`
/// This is legal since by the official java docs, users cannot extend `MethodHandle` so we can
/// strictly rely upon our own implementation.
#[derive(Debug, Clone)]
pub struct MethodHandleInstance {
    pub(crate) inner: ClassInstance,
    /// This should be kept in sync with the `method_type_ref` field
    pub typ: MethodHandleType,
    /// Lazily initialized reference to the `MethodType` instance
    pub method_type_ref: Option<GcRef<ClassInstance>>,
}
impl MethodHandleInstance {
    #[must_use]
    pub fn new(inner_instance: ClassInstance, typ: MethodHandleType) -> MethodHandleInstance {
        MethodHandleInstance {
            inner: inner_instance,
            typ,
            method_type_ref: None,
        }
    }
}
impl_reference_instance_conv!(MethodHandle => MethodHandleInstance);
impl MemorySize for MethodHandleInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl GcValueMarker for MethodHandleInstance {}

/// Note: This is only for `rho/invoke/MethodHandleInfoInst`
/// Since `MethodHandleInfo` is an interface, theoretically anything can implement it
#[derive(Debug, Clone)]
pub struct MethodHandleInfoInstance {
    pub(crate) inner: ClassInstance,
    pub method_handle: GcRef<MethodHandleInstance>,
}
impl MethodHandleInfoInstance {
    #[must_use]
    pub fn new(
        inner_instance: ClassInstance,
        method_handle: GcRef<MethodHandleInstance>,
    ) -> MethodHandleInfoInstance {
        MethodHandleInfoInstance {
            inner: inner_instance,
            method_handle,
        }
    }
}
impl_reference_instance_conv!(MethodHandleInfo => MethodHandleInfoInstance);
impl MemorySize for MethodHandleInfoInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl GcValueMarker for MethodHandleInfoInstance {}

/// An instance of some class  
/// Note: If you want a `Class<T>` instance, then you want [`StaticFormInstance`]
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
    #[must_use]
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
impl_reference_instance_conv!(Class => ClassInstance);
impl MemorySize for ClassInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl GcValueMarker for ClassInstance {}

/// The class that is the static class, which may have its own fields and the like on it
#[derive(Debug, Clone)]
pub struct StaticClassInstance {
    /// Its own id
    pub id: ClassId,
    /// Static fields
    pub fields: Fields,
    pub form: Option<GcRef<StaticFormInstance>>,
}
impl StaticClassInstance {
    pub(crate) fn new(id: ClassId, fields: Fields) -> StaticClassInstance {
        StaticClassInstance {
            id,
            fields,
            form: None,
        }
    }
}
impl_instance_conv!(StaticClass => StaticClassInstance);
impl MemorySize for StaticClassInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl GcValueMarker for StaticClassInstance {}

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
impl_reference_instance_conv!(PrimitiveArray => PrimitiveArrayInstance);
impl MemorySize for PrimitiveArrayInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl GcValueMarker for PrimitiveArrayInstance {}

#[derive(Debug, Clone)]
pub struct ReferenceArrayInstance {
    pub instanceof: ClassId,
    pub element_type: ClassId,
    pub elements: Vec<Option<GcRef<ReferenceInstance>>>,
    // TODO: This only exists so that we can return an empty iterator over it that is the same type
    // as those returned in the impl iterator for instance. We should simply make an empty version.
    fields: Fields,
}
impl ReferenceArrayInstance {
    #[must_use]
    pub fn new(
        instanceof: ClassId,
        element_type: ClassId,
        elements: Vec<Option<GcRef<ReferenceInstance>>>,
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
impl_reference_instance_conv!(ReferenceArray => ReferenceArrayInstance);
impl MemorySize for ReferenceArrayInstance {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl GcValueMarker for ReferenceArrayInstance {}

/// Will not be [`u16::MAX`]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct FieldIndex(u16);
impl FieldIndex {
    #[must_use]
    pub(crate) fn new_unchecked(v: u16) -> FieldIndex {
        debug_assert_ne!(v, u16::MAX);
        FieldIndex(v)
    }

    #[must_use]
    pub(crate) fn get(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldId {
    class_id: ClassId,
    field_index: FieldIndex,
}
impl FieldId {
    #[must_use]
    pub fn unchecked_compose(class_id: ClassId, field_index: FieldIndex) -> Self {
        Self {
            class_id,
            field_index,
        }
    }

    #[must_use]
    pub fn decompose(self) -> (ClassId, FieldIndex) {
        (self.class_id, self.field_index)
    }
}
#[derive(Default, Debug, Clone)]
pub struct Fields {
    /// Stores the id of the class and its index
    /// because a class can have a field name 'a' and extend a class with a field named 'a'
    /// and they are different fields.
    fields: HashMap<FieldId, Field>,
}
impl Fields {
    #[must_use]
    pub fn get(&self, id: FieldId) -> Option<&Field> {
        self.fields.get(&id)
    }

    #[must_use]
    pub fn get_mut(&mut self, id: FieldId) -> Option<&mut Field> {
        self.fields.get_mut(&id)
    }

    pub fn insert(&mut self, id: FieldId, field: Field) {
        self.fields.insert(id, field);
    }

    pub fn iter(&self) -> impl Iterator<Item = (FieldId, &Field)> {
        self.fields.iter().map(|x| (*x.0, x.1))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (FieldId, &mut Field)> {
        self.fields.iter_mut().map(|x| (*x.0, x.1))
    }
}

pub type FieldType = RuntimeType<ClassId>;

/// A field with some value
/// This does not keep track of the name
/// It also does not keep track of whether it is static because that is decided by the outside
#[derive(Debug, Clone)]
pub struct Field {
    value: RuntimeValue,
    // FIXME: Make the GC look into field type's for the class id
    /// The type of the field
    /// Value should be able to be reasonably treated as this
    /// This is stored because it shouldn't be modified at runtime anyway, so we don't have to
    /// bother looking it up in the class file each time.
    /// We store the class id instead of a gcref to the static class (for references)
    /// because doing the circular initialization is a bit of a paint
    typ: FieldType,
    /// Whether it is a final value or not, so it cannot change after initialization
    is_final: bool,
    access: FieldAccess,
}
impl Field {
    pub(crate) fn new(
        value: RuntimeValue,
        typ: FieldType,
        is_final: bool,
        access: FieldAccess,
    ) -> Field {
        Field {
            value,
            typ,
            is_final,
            access,
        }
    }

    #[must_use]
    pub fn value(&self) -> RuntimeValue {
        self.value
    }

    #[must_use]
    pub fn value_mut(&mut self) -> &mut RuntimeValue {
        &mut self.value
    }

    #[must_use]
    pub fn typ(&self) -> FieldType {
        self.typ
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
