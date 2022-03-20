use std::{borrow::Cow, ops::Range, rc::Rc};

use classfile_parser::{
    constant_info::{ClassConstant, ConstantInfo, Utf8Constant},
    constant_pool::{ConstantPoolIndex, ConstantPoolIndexRaw},
    field_info::FieldInfoOpt,
    method_info::{MethodInfo, MethodInfoOpt},
    parser::ParseData,
    ClassFileOpt, ClassFileVersion, LoadError,
};

pub use classfile_parser::ClassAccessFlags;

use crate::{
    code::types::PrimitiveType,
    data::class_names::ClassNames,
    id::{ClassId, ExactMethodId, MethodIndex, PackageId},
    util::format_class_as_object_desc,
    BadIdError,
};

#[derive(Debug, Clone)]
pub enum ClassFileIndexError {
    InvalidThisClassIndex,
    InvalidThisClassNameIndex,
    InvalidSuperClassIndex,
    InvalidSuperClassNameIndex,
}

#[derive(Debug, Clone)]
pub struct ClassFileData {
    pub(crate) id: ClassId,
    /// The raw bytes of the class file
    /// We keep this around because the majority of class files are relatively small
    /// This could switch to holding a File, or just opening the file as needed, to read the bytes
    /// out (that we haven't parsed and collected, because doing that for everything is excessive)
    /// As well, an optimization for memory could throw away parts that we always parse, but that
    /// complicates the implementation, and so has not yet been done.
    pub(crate) class_file_data: Rc<[u8]>,
    pub(crate) class_file: ClassFileOpt,
}
impl ClassFileData {
    #[must_use]
    pub fn new(id: ClassId, class_file_data: Rc<[u8]>, class_file: ClassFileOpt) -> ClassFileData {
        ClassFileData {
            id,
            class_file_data,
            class_file,
        }
    }

    #[must_use]
    /// Gets the classfile directly.
    /// There is _no_ guarantee that this is stable, and it may be removed without a major version
    /// change.
    pub fn get_class_file_unstable(&self) -> &ClassFileOpt {
        &self.class_file
    }

    // TODO: Give the class file a good way of parsing attributes to not expose
    // implementation details
    #[must_use]
    pub fn parse_data_for(&self, r: Range<usize>) -> ParseData {
        ParseData::from_range(&self.class_file_data, r)
    }

    #[must_use]
    pub fn id(&self) -> ClassId {
        self.id
    }

    #[must_use]
    pub fn version(&self) -> Option<ClassFileVersion> {
        Some(self.class_file.version)
    }

    pub fn get_t<'a, T>(&'a self, i: impl TryInto<ConstantPoolIndex<T>>) -> Option<&'a T>
    where
        &'a T: TryFrom<&'a ConstantInfo>,
    {
        self.class_file.const_pool.get_t(i)
    }

    // TODO: Add a cache for these!
    pub fn get_text_t(&self, i: impl TryInto<ConstantPoolIndex<Utf8Constant>>) -> Option<Cow<str>> {
        self.get_t(i).map(|x| x.as_text(&self.class_file_data))
    }

    pub fn get_text_b(&self, i: impl TryInto<ConstantPoolIndex<Utf8Constant>>) -> Option<&[u8]> {
        self.get_t(i).map(|x| x.as_bytes(&self.class_file_data))
    }

    #[must_use]
    pub fn load_attribute_range_with_name(&self, name: &str) -> Option<Range<usize>> {
        self.class_file
            .load_attribute_with_name(&self.class_file_data, name)
            .ok()
            .flatten()
    }

    pub fn load_method_info_by_index(
        &self,
        index: MethodIndex,
    ) -> Result<Cow<MethodInfo>, LoadError> {
        self.class_file.load_method_at(&self.class_file_data, index)
    }

    pub fn load_method_info_opt_by_index(
        &self,
        index: MethodIndex,
    ) -> Result<MethodInfoOpt, LoadError> {
        self.class_file
            .load_method_opt_at(&self.class_file_data, index)
    }

    pub fn load_method_info_opt_iter_with_index(
        &self,
    ) -> impl Iterator<Item = (MethodIndex, MethodInfoOpt)> + '_ {
        // The number of methods from the file will always be less than a u16
        #[allow(clippy::cast_possible_truncation)]
        self.load_method_info_opt_iter()
            .enumerate()
            .map(|(i, info)| (i as u16, info))
    }

    /// This is guaranteed to be in order
    pub fn load_method_info_opt_iter(&self) -> impl Iterator<Item = MethodInfoOpt> + '_ {
        self.class_file.load_method_opt_iter(&self.class_file_data)
    }

    /// Load all the methods from the class file into memory
    /// This should be used if you're going to be iterating over all/most methods
    /// Since the individual seeking methods would be slower if they were not laoded at all
    pub fn load_all_methods_backing(&mut self) -> Result<(), LoadError> {
        self.class_file.load_all_methods_mut(&self.class_file_data)
    }

    #[must_use]
    // TODO: Give this a better name and/or make it more consistent with the underlying version
    pub fn load_method_attribute_info_range_by_name(
        &self,
        index: MethodIndex,
        name: &str,
    ) -> Option<Range<usize>> {
        self.class_file
            .load_method_attribute_info_at_with_name(&self.class_file_data, index, name)
            .ok()
            .flatten()
    }

    pub fn load_field_values_iter(
        &self,
    ) -> impl Iterator<
        Item = Result<(FieldInfoOpt, Option<ConstantPoolIndexRaw<ConstantInfo>>), LoadError>,
    > + '_ {
        self.class_file
            .load_fields_values_iter(&self.class_file_data)
    }

    #[must_use]
    pub fn methods_len(&self) -> u16 {
        self.class_file.methods.len() as u16
    }

    // #[must_use]
    // pub fn get_method(&self, index: usize) -> Option<&MethodInfo> {
    //     self.class_file.methods.get(index)
    // }

    // #[must_use]
    // pub fn methods(&self) -> &[MethodInfo] {
    //     self.class_file.methods.as_slice()
    // }

    #[must_use]
    pub fn access_flags(&self) -> ClassAccessFlags {
        self.class_file.access_flags
    }

    pub(crate) fn get_this_class_name(&self) -> Result<&[u8], ClassFileIndexError> {
        let this_class = self
            .get_t(self.class_file.this_class)
            .ok_or(ClassFileIndexError::InvalidThisClassIndex)?;
        self.get_text_b(this_class.name_index)
            .ok_or(ClassFileIndexError::InvalidThisClassNameIndex)
    }

    pub(crate) fn get_super_class_name(&self) -> Result<Option<&[u8]>, ClassFileIndexError> {
        // There is no base class
        // Only java/lang/Object should have no base class, but we don't do that verification here
        if self.class_file.super_class.is_zero() {
            return Ok(None);
        }

        let super_class = self
            .get_t(self.class_file.super_class)
            .ok_or(ClassFileIndexError::InvalidSuperClassIndex)?;
        self.get_text_b(super_class.name_index)
            .map(Some)
            .ok_or(ClassFileIndexError::InvalidSuperClassNameIndex)
    }

    pub(crate) fn get_super_class_id(
        &self,
        class_names: &mut ClassNames,
    ) -> Result<Option<ClassId>, ClassFileIndexError> {
        Ok(self
            .get_super_class_name()?
            .map(|x| class_names.gcid_from_bytes(x)))
    }

    pub fn interfaces_indices_iter(
        &self,
    ) -> impl Iterator<Item = ConstantPoolIndexRaw<ClassConstant>> + '_ {
        self.class_file.interfaces.iter().copied()
    }
}

#[derive(Debug, Clone)]
pub enum ClassVariant {
    Class(Class),
    Array(ArrayClass),
}
impl ClassVariant {
    #[must_use]
    pub fn id(&self) -> ClassId {
        match self {
            Self::Class(x) => x.id,
            Self::Array(x) => x.id,
        }
    }

    #[must_use]
    pub fn super_id(&self) -> Option<ClassId> {
        match self {
            Self::Class(x) => x.super_id(),
            Self::Array(x) => Some(x.super_id()),
        }
    }

    #[must_use]
    pub fn access_flags(&self) -> ClassAccessFlags {
        match self {
            Self::Class(x) => x.access_flags,
            Self::Array(x) => x.access_flags,
        }
    }

    #[must_use]
    /// Returns the id of the package that contains this class
    /// `None` means that it is the topmost package
    pub fn package(&self) -> Option<PackageId> {
        match self {
            ClassVariant::Class(x) => x.package(),
            ClassVariant::Array(x) => x.package(),
        }
    }

    #[must_use]
    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    #[must_use]
    pub fn as_class(&self) -> Option<&Class> {
        match self {
            Self::Class(x) => Some(x),
            Self::Array(_) => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&ArrayClass> {
        match self {
            Self::Class(_) => None,
            Self::Array(x) => Some(x),
        }
    }

    #[must_use]
    pub fn is_interface(&self) -> bool {
        match self {
            ClassVariant::Class(x) => x.is_interface(),
            ClassVariant::Array(_) => false,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Class {
    pub(crate) id: ClassId,
    pub(crate) super_class: Option<ClassId>,
    pub(crate) package: Option<PackageId>,
    pub(crate) access_flags: ClassAccessFlags,
    /// This is just the length of methods
    /// Not all methods are guaranteed to be initialized
    /// 0..last_method_id
    pub(crate) len_method_idx: MethodIndex,
}
impl Class {
    pub(crate) fn new(
        id: ClassId,
        super_class: Option<ClassId>,
        package: Option<PackageId>,
        access_flags: ClassAccessFlags,
        len_method_idx: MethodIndex,
    ) -> Self {
        Self {
            id,
            super_class,
            package,
            access_flags,
            len_method_idx,
        }
    }

    #[must_use]
    pub fn id(&self) -> ClassId {
        self.id
    }

    #[must_use]
    pub fn super_id(&self) -> Option<ClassId> {
        self.super_class
    }

    #[must_use]
    pub fn package(&self) -> Option<PackageId> {
        self.package
    }

    #[must_use]
    pub fn is_interface(&self) -> bool {
        self.access_flags.contains(ClassAccessFlags::INTERFACE)
    }

    /// Iterate over all method ids that this method has.
    /// Note that this is just the ids, they are not guaranteed to be loaded.
    pub fn iter_method_ids(&self) -> impl Iterator<Item = ExactMethodId> {
        let class_id = self.id;
        (0..self.len_method_idx).map(move |idx| ExactMethodId::unchecked_compose(class_id, idx))
    }
}

// TODO: Are arrays in the same package as their defining type?
#[derive(Debug, Clone)]
pub struct ArrayClass {
    pub(crate) id: ClassId,
    pub(crate) component_type: ArrayComponentType,
    /// Always "java/lang/Object"
    pub(crate) super_class: ClassId,
    pub(crate) access_flags: ClassAccessFlags,
    /// The package id of the innermost component type, if it has one
    pub(crate) package: Option<PackageId>,
}
impl ArrayClass {
    // TODO: provide more libsound ways of creating this
    #[must_use]
    pub fn new_unchecked(
        id: ClassId,
        component_type: ArrayComponentType,
        super_class: ClassId,
        access_flags: ClassAccessFlags,
        package: Option<PackageId>,
    ) -> Self {
        ArrayClass {
            id,
            component_type,
            super_class,
            access_flags,
            package,
        }
    }

    #[must_use]
    /// These are cesu8 valid strings
    pub fn get_interface_names() -> &'static [&'static [u8]] {
        &[b"java/lang/Cloneable", b"java/io/Serializable"]
    }

    #[must_use]
    pub fn id(&self) -> ClassId {
        self.id
    }

    #[must_use]
    pub fn component_type(&self) -> ArrayComponentType {
        self.component_type.clone()
    }

    #[must_use]
    pub fn super_id(&self) -> ClassId {
        self.super_class
    }

    #[must_use]
    /// Returns the package id
    /// If there is none, then it is of some class that is rootmost package
    pub fn package(&self) -> Option<PackageId> {
        self.package
    }
}

/// NOTE: We could have various other types, like unsigned versions, to allow for more granular type
/// checking, but that only makes sense if it can be determined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayComponentType {
    Boolean,
    Char,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    Class(ClassId),
}
impl ArrayComponentType {
    #[must_use]
    pub fn is_primitive(&self) -> bool {
        !matches!(self, ArrayComponentType::Class(_))
    }

    #[must_use]
    /// Convert to class id if it is of the `Class` variant, aka if it is non-Primitive
    pub fn into_class_id(self) -> Option<ClassId> {
        match self {
            ArrayComponentType::Class(id) => Some(id),
            _ => None,
        }
    }

    pub fn to_desc_string(&self, class_names: &mut ClassNames) -> Result<Vec<u8>, BadIdError> {
        match self {
            ArrayComponentType::Byte => Ok(Vec::from(b"B" as &[u8])),
            ArrayComponentType::Char => Ok(Vec::from(b"C" as &[u8])),
            ArrayComponentType::Double => Ok(Vec::from(b"D" as &[u8])),
            ArrayComponentType::Float => Ok(Vec::from(b"F" as &[u8])),
            ArrayComponentType::Int => Ok(Vec::from(b"I" as &[u8])),
            ArrayComponentType::Long => Ok(Vec::from(b"J" as &[u8])),
            ArrayComponentType::Class(class_id) => {
                let (class_name, class_info) = class_names.name_from_gcid(*class_id)?;
                if class_info.is_array() {
                    // If we have the id for an array then we just use the singular path it has
                    // because writing it as an object is incorrect.
                    Ok(class_name.get().to_owned())
                } else {
                    Ok(format_class_as_object_desc(class_name.get()))
                }
            }
            ArrayComponentType::Short => Ok(Vec::from(b"S" as &[u8])),
            ArrayComponentType::Boolean => Ok(Vec::from(b"Z" as &[u8])),
        }
    }
}
// TODO: Make From<DescriptorTypeBasic>
impl From<PrimitiveType> for ArrayComponentType {
    fn from(prim: PrimitiveType) -> ArrayComponentType {
        match prim {
            PrimitiveType::Byte | PrimitiveType::UnsignedByte => ArrayComponentType::Byte,
            PrimitiveType::Short | PrimitiveType::UnsignedShort => ArrayComponentType::Short,
            PrimitiveType::Int => ArrayComponentType::Int,
            PrimitiveType::Long => ArrayComponentType::Long,
            PrimitiveType::Float => ArrayComponentType::Float,
            PrimitiveType::Double => ArrayComponentType::Double,
            PrimitiveType::Char => ArrayComponentType::Char,
            PrimitiveType::Boolean => ArrayComponentType::Boolean,
        }
    }
}
