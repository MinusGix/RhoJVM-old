use std::{borrow::Cow, ops::Range, path::PathBuf};

use classfile_parser::{
    constant_info::{ClassConstant, ConstantInfo, Utf8Constant},
    constant_pool::{ConstantPoolIndex, ConstantPoolIndexRaw},
    method_info::{MethodInfo, MethodInfoOpt},
    parser::ParseData,
    ClassFileOpt, ClassFileVersion,
};

pub use classfile_parser::ClassAccessFlags;

use crate::{
    code::{method::Method, types::PrimitiveType},
    id::{ClassFileId, ClassId, MethodId, MethodIndex, PackageId},
    BadIdError, ClassNames, LoadMethodError, Methods,
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
    pub(crate) id: ClassFileId,
    #[allow(dead_code)]
    /// The direct path to the file
    pub(crate) path: PathBuf,
    /// The raw bytes of the class file
    /// We keep this around because the majority of class files are relatively small
    /// This could switch to holding a File, or just opening the file as needed, to read the bytes
    /// out (that we haven't parsed and collected, because doing that for everything is excessive)
    /// As well, an optimization for memory could throw away parts that we always parse, but that
    /// complicates the implementation, and so has not yet been done.
    pub(crate) class_file_data: Vec<u8>,
    pub(crate) class_file: ClassFileOpt,
}
impl ClassFileData {
    #[must_use]
    /// Gets the classfile directly.
    /// There is _no_ guarantee that this is stable, and it may be removed without a major version
    /// change.
    pub fn get_class_file_unstable(&self) -> &ClassFileOpt {
        &self.class_file
    }

    // TODO: Give the class file a good way of parsing attributes to not expose
    // implementation details
    pub(crate) fn parse_data_for(&self, r: Range<usize>) -> ParseData {
        ParseData::from_range(&self.class_file_data, r)
    }

    #[must_use]
    pub fn id(&self) -> ClassFileId {
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

    pub fn get_t_mut<'a, T>(
        &'a mut self,
        i: impl TryInto<ConstantPoolIndex<T>>,
    ) -> Option<&'a mut T>
    where
        &'a mut T: TryFrom<&'a mut ConstantInfo>,
    {
        self.class_file.const_pool.get_t_mut(i)
    }

    // TODO: Add a cache for these!
    pub fn get_text_t(&self, i: impl TryInto<ConstantPoolIndex<Utf8Constant>>) -> Option<Cow<str>> {
        self.get_t(i).map(|x| x.as_text(&self.class_file_data))
    }

    #[must_use]
    pub fn load_method_info_by_index(&self, index: MethodIndex) -> Option<Cow<MethodInfo>> {
        self.class_file.load_method_at(&self.class_file_data, index)
    }

    #[must_use]
    pub fn load_method_info_opt_by_index(&self, index: MethodIndex) -> Option<MethodInfoOpt> {
        self.class_file
            .load_method_opt_at(&self.class_file_data, index)
    }

    /// This is guaranteed to be in order
    pub fn load_method_info_opt_iter(&self) -> impl Iterator<Item = MethodInfoOpt> + '_ {
        self.class_file.load_method_opt_iter(&self.class_file_data)
    }

    /// Load all the methods from the class file into memory
    /// This should be used if you're going to be iterating over all/most methods
    /// Since the individual seeking methods would be slower if they were not laoded at all
    pub fn load_all_methods_backing(&mut self) {
        self.class_file.load_all_methods_mut(&self.class_file_data);
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

    pub(crate) fn get_this_class_name(&self) -> Result<Cow<str>, ClassFileIndexError> {
        let this_class = self
            .get_t(self.class_file.this_class)
            .ok_or(ClassFileIndexError::InvalidThisClassIndex)?;
        self.get_text_t(this_class.name_index)
            .ok_or(ClassFileIndexError::InvalidThisClassNameIndex)
    }

    pub(crate) fn get_super_class_name(&self) -> Result<Option<Cow<str>>, ClassFileIndexError> {
        // There is no base class
        // Only java/lang/Object should have no base class, but we don't do that verification here
        if self.class_file.super_class.is_zero() {
            return Ok(None);
        }

        let super_class = self
            .get_t(self.class_file.super_class)
            .ok_or(ClassFileIndexError::InvalidSuperClassIndex)?;
        self.get_text_t(super_class.name_index)
            .map(Some)
            .ok_or(ClassFileIndexError::InvalidSuperClassNameIndex)
    }

    pub(crate) fn get_super_class_id(
        &self,
        class_names: &mut ClassNames,
    ) -> Result<Option<ClassFileId>, ClassFileIndexError> {
        Ok(self
            .get_super_class_name()?
            .map(|x| class_names.gcid_from_str(x)))
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
}
#[derive(Debug, Clone)]
pub struct Class {
    pub(crate) id: ClassId,
    pub(crate) super_class: Option<ClassFileId>,
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
        super_class: Option<ClassFileId>,
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
    pub fn super_id(&self) -> Option<ClassFileId> {
        self.super_class
    }

    #[must_use]
    pub fn package(&self) -> Option<PackageId> {
        self.package
    }

    /// Iterate over all method ids that this method has.
    /// Note that this is just the ids, they are not guaranteed to be loaded.
    pub fn iter_method_ids(&self) -> impl Iterator<Item = MethodId> {
        let class_id = self.id;
        (0..self.len_method_idx).map(move |idx| MethodId::unchecked_compose(class_id, idx))
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
}
impl ArrayClass {
    // TODO: provide more libsound ways of creating this
    #[must_use]
    pub fn new_unchecked(
        id: ClassId,
        component_type: ArrayComponentType,
        super_class: ClassId,
        access_flags: ClassAccessFlags,
    ) -> Self {
        ArrayClass {
            id,
            component_type,
            super_class,
            access_flags,
        }
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
}

/// NOTE: We could have various other types, like unsigned versions, to allow for more granular type
/// checking, but that only makes sense if it can be determined.
#[derive(Debug, Clone)]
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

    pub fn to_desc_string(&self, class_names: &mut ClassNames) -> Result<String, BadIdError> {
        match self {
            ArrayComponentType::Byte => Ok("B".to_owned()),
            ArrayComponentType::Char => Ok("C".to_owned()),
            ArrayComponentType::Double => Ok("D".to_owned()),
            ArrayComponentType::Float => Ok("F".to_owned()),
            ArrayComponentType::Int => Ok("I".to_owned()),
            ArrayComponentType::Long => Ok("J".to_owned()),
            ArrayComponentType::Class(class_id) => {
                let name = class_names.name_from_gcid(*class_id)?;
                let path = name.path();
                if name.is_array() {
                    // If we have the id for an array then we just use the singular path it has
                    // because writing it as an object is incorrect.
                    Ok(path.to_owned())
                } else {
                    Ok(format!("L{};", path))
                }
            }
            ArrayComponentType::Short => Ok("S".to_owned()),
            ArrayComponentType::Boolean => Ok("Z".to_owned()),
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
