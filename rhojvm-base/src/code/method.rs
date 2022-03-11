use std::borrow::Cow;
use std::num::NonZeroUsize;

use classfile_parser::{
    attribute_info::code_attribute_parser,
    constant_info::Utf8Constant,
    constant_pool::ConstantPoolIndexRaw,
    descriptor::{
        method::{
            MethodDescriptor as MethodDescriptorCF, MethodDescriptorError,
            MethodDescriptorParserIterator as MethodDescriptorParserIteratorCF,
        },
        DescriptorType as DescriptorTypeCF, DescriptorTypeBasic as DescriptorTypeBasicCF,
    },
    method_info::{MethodAccessFlags, MethodInfoOpt},
    ClassAccessFlags,
};
use either::Either;
use smallvec::SmallVec;

use crate::{
    class::{ArrayComponentType, ClassFileData},
    code::{self},
    data::{class_files::ClassFiles, class_names::ClassNames},
    id::{ClassId, MethodId},
    util::format_class_as_object_desc,
    BadIdError, LoadCodeError, LoadMethodError, StepError, VerifyMethodError,
};

use super::CodeInfo;

// TODO: We could have a MethodAlias (so Method becomes an enum of the current Method and
// MethodAlias), which simply has the id of another method. This would then allow us to have
// a method to look for duplicates, and to limit generation of duplicates.
// but this does complicate the code somewhat and make more checks needed.
// especially to avoid circularity
// There could be a separate `method_aliases: HashMap<MethodId, MethodAlias>` on `ProgramInfo`,
// and while that helps reduce size (since MethodAlias is probably just an alias_id), and avoid
// circularity (if we only lookup an alias in methods), it does complicate matters and be more
// random memory accesses?

#[derive(Debug, Clone)]
pub struct Method {
    /// Its own id.
    pub(crate) id: MethodId,
    /// Parameters and return type of the methods
    pub(crate) descriptor: MethodDescriptor,
    /// The access flags for the method
    pub(crate) access_flags: MethodAccessFlags,
    /// The methods that are overridden by this.
    /// `None` if it has not been initialized
    pub(crate) overrides: Option<SmallVec<[MethodOverride; 2]>>,
    pub(crate) code: Option<CodeInfo>,
    pub(crate) is_init: bool,
    pub(crate) name_index: ConstantPoolIndexRaw<Utf8Constant>,
}
impl Method {
    pub(crate) fn new(
        id: MethodId,
        is_init: bool,
        descriptor: MethodDescriptor,
        access_flags: MethodAccessFlags,
        name_index: ConstantPoolIndexRaw<Utf8Constant>,
    ) -> Self {
        Self {
            id,
            is_init,
            descriptor,
            access_flags,
            overrides: None,
            code: None,
            name_index,
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    /// Construct the method with an already known name
    /// NOTE: This should _always_ be the same as the method's actual name.
    pub(crate) fn new_from_info(
        id: MethodId,
        class_file: &ClassFileData,
        class_names: &mut ClassNames,
        method: MethodInfoOpt,
    ) -> Result<Self, LoadMethodError> {
        let descriptor_text = class_file.get_text_b(method.descriptor_index).ok_or(
            LoadMethodError::InvalidDescriptorIndex {
                index: method.descriptor_index,
            },
        )?;
        let desc = MethodDescriptor::from_text(descriptor_text, class_names)
            .map_err(LoadMethodError::MethodDescriptorError)?;

        let name_index = method.name_index;
        // TODO: We could do slightly better by just getting a slice of the bytes and then
        // comparing it to <init>
        let method_name = class_file.get_text_t(method.name_index).ok_or(
            LoadMethodError::InvalidMethodNameIndex {
                index: method.name_index,
            },
        )?;
        let is_init = method_name == Cow::Borrowed("<init>");
        Ok(Method::new(
            id,
            is_init,
            desc,
            method.access_flags,
            name_index,
        ))
    }

    #[must_use]
    pub fn name_index(&self) -> ConstantPoolIndexRaw<Utf8Constant> {
        self.name_index
    }

    #[must_use]
    pub fn id(&self) -> MethodId {
        self.id
    }

    #[must_use]
    /// Whether or not it is an <init> function
    pub fn is_init(&self) -> bool {
        self.is_init
    }

    #[must_use]
    pub fn descriptor(&self) -> &MethodDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub fn access_flags(&self) -> MethodAccessFlags {
        self.access_flags
    }

    #[must_use]
    /// Some if it has been initialized
    pub fn overrides(&self) -> Option<&[MethodOverride]> {
        self.overrides.as_deref()
    }

    #[must_use]
    /// Some if it has been initialized
    pub fn code(&self) -> Option<&CodeInfo> {
        self.code.as_ref()
    }

    #[must_use]
    /// Take the code info from this so that you can use it directly
    pub fn take_code_info(&mut self) -> Option<CodeInfo> {
        self.code.take()
    }

    /// Insert the code info to be used. This will replace the code if it hasn't already been loaded
    /// This _must_ have come from `take_code_info` from the same method from the same class file
    /// It _must_ not have been modified.
    /// If the method should not have code, then you should not insert code.
    pub fn unchecked_insert_code(&mut self, code: CodeInfo) {
        self.code = Some(code);
    }

    #[must_use]
    /// Whether the method should have code or not.
    /// Note that this does not determine if there actually is code, there could be a malformed
    /// class file, but it does tell us if there _should_ be.
    pub fn should_have_code(&self) -> bool {
        // native and abstract methods do not have code
        !self.access_flags.contains(MethodAccessFlags::NATIVE)
            && !self.access_flags.contains(MethodAccessFlags::ABSTRACT)
    }

    pub fn verify_access_flags(&self) -> Result<(), VerifyMethodError> {
        verify_method_access_flags(self.access_flags)
    }

    /// The class file must be the class file that contains this method
    pub fn load_code_with_unchecked(
        &mut self,
        class_file: &ClassFileData,
    ) -> Result<(), StepError> {
        if let Some(code) = self.direct_load_code_with_unchecked(class_file)? {
            self.code = Some(code);
        }

        Ok(())
    }

    /// Loads code if it isn't already loaded and exists
    /// The class that contains the method should already be loaded
    pub fn load_code(&mut self, class_files: &mut ClassFiles) -> Result<(), StepError> {
        let (class_id, _) = self.id().decompose();

        let class_file = class_files
            .get(&class_id)
            .ok_or(StepError::MissingLoadedValue(
                "load_method_code : class_file",
            ))?;

        self.load_code_with_unchecked(class_file)
    }

    fn direct_load_code_with_unchecked(
        &self,
        class_file: &ClassFileData,
    ) -> Result<Option<CodeInfo>, StepError> {
        debug_assert_eq!(self.id().decompose().0, class_file.id());

        // TODO: Check for code for native/abstract methods to allow malformed
        // versions of them?
        if !self.should_have_code() {
            return Ok(None);
        }

        if self.code().is_some() {
            // It already loaded
            return Ok(None);
        }

        let (_, method_index) = self.id.decompose();
        let code_attr_range =
            class_file.load_method_attribute_info_range_by_name(method_index, "Code");

        if let Some(code_attr_range) = code_attr_range {
            let (data_rem, code_attr) =
                code_attribute_parser(class_file.parse_data_for(code_attr_range))
                    .map_err(|_| LoadCodeError::InvalidCodeAttribute)?;
            debug_assert!(data_rem.is_empty(), "The remaining data after parsing the code attribute was non-empty. This indicates a bug.");

            // TODO: A config for code parsing that includes information like the class file
            // version?
            // or we could _try_ making it not care and make that a verification step?
            let code =
                code::parse_code(code_attr, class_file).map_err(LoadCodeError::InstructionParse)?;

            return Ok(Some(code));
        }

        Ok(None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorTypeBasic {
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Class(ClassId),
    Short,
    Boolean,
}
impl DescriptorTypeBasic {
    /// Convert to a string used in a descriptor
    pub fn to_desc_string(self, class_names: &mut ClassNames) -> Result<Vec<u8>, BadIdError> {
        match self {
            DescriptorTypeBasic::Byte => Ok(Vec::from(b"B" as &[u8])),
            DescriptorTypeBasic::Char => Ok(Vec::from(b"C" as &[u8])),
            DescriptorTypeBasic::Double => Ok(Vec::from(b"D" as &[u8])),
            DescriptorTypeBasic::Float => Ok(Vec::from(b"F" as &[u8])),
            DescriptorTypeBasic::Int => Ok(Vec::from(b"I" as &[u8])),
            DescriptorTypeBasic::Long => Ok(Vec::from(b"J" as &[u8])),
            DescriptorTypeBasic::Class(class_id) => {
                let (class_name, class_info) = class_names.name_from_gcid(class_id)?;
                if class_info.is_array() {
                    // If we have the id for an array then we just use the singular path it has
                    // because writing it as an object is incorrect.
                    Ok(class_name.get().to_owned())
                } else {
                    Ok(format_class_as_object_desc(class_name.get()))
                }
            }
            DescriptorTypeBasic::Short => Ok(Vec::from(b"S" as &[u8])),
            DescriptorTypeBasic::Boolean => Ok(Vec::from(b"Z" as &[u8])),
        }
    }

    /// Returns an iterator over the desc type
    /// Most of the returned strings are static, but class would have one that is owned by names
    pub(crate) fn as_desc_iter(
        self,
        class_names: &ClassNames,
    ) -> Result<
        Either<impl Iterator<Item = &'_ [u8]> + Clone, impl Iterator<Item = &'_ [u8]> + Clone>,
        BadIdError,
    > {
        Ok(Either::Left(match self {
            DescriptorTypeBasic::Byte => [b"B" as &[u8]].into_iter(),
            DescriptorTypeBasic::Char => [b"C" as &[u8]].into_iter(),
            DescriptorTypeBasic::Double => [b"D" as &[u8]].into_iter(),
            DescriptorTypeBasic::Float => [b"F" as &[u8]].into_iter(),
            DescriptorTypeBasic::Int => [b"I" as &[u8]].into_iter(),
            DescriptorTypeBasic::Long => [b"J" as &[u8]].into_iter(),
            DescriptorTypeBasic::Short => [b"S" as &[u8]].into_iter(),
            DescriptorTypeBasic::Boolean => [b"Z" as &[u8]].into_iter(),
            DescriptorTypeBasic::Class(class_id) => {
                let (class_name, class_info) = class_names.name_from_gcid(class_id)?;
                if class_info.is_array() {
                    // Arrays already have leading [
                    [class_name.get()].into_iter()
                } else {
                    return Ok(Either::Right([b"L", class_name.get(), b";"].into_iter()));
                }
            }
        }))
    }

    pub(crate) fn from_class_file_desc(
        desc: DescriptorTypeBasicCF<'_>,
        class_names: &mut ClassNames,
    ) -> Self {
        match desc {
            DescriptorTypeBasicCF::Byte => Self::Byte,
            DescriptorTypeBasicCF::Char => Self::Char,
            DescriptorTypeBasicCF::Double => Self::Double,
            DescriptorTypeBasicCF::Float => Self::Float,
            DescriptorTypeBasicCF::Int => Self::Int,
            DescriptorTypeBasicCF::Long => Self::Long,
            DescriptorTypeBasicCF::ClassName(name) => Self::Class(class_names.gcid_from_cow(name)),
            DescriptorTypeBasicCF::Short => Self::Short,
            DescriptorTypeBasicCF::Boolean => Self::Boolean,
        }
    }

    pub(crate) fn name(self) -> Option<&'static str> {
        Some(match self {
            DescriptorTypeBasic::Byte => "byte",
            DescriptorTypeBasic::Char => "char",
            DescriptorTypeBasic::Double => "double",
            DescriptorTypeBasic::Float => "float",
            DescriptorTypeBasic::Int => "int",
            DescriptorTypeBasic::Long => "long",
            DescriptorTypeBasic::Class(_) => return None,
            DescriptorTypeBasic::Short => "short",
            DescriptorTypeBasic::Boolean => "boolean",
        })
    }

    pub(crate) fn access_flags(self) -> Option<ClassAccessFlags> {
        match self {
            DescriptorTypeBasic::Class(_) => None,
            _ => Some(ClassAccessFlags::PUBLIC),
        }
    }

    pub(crate) fn as_array_component_type(self) -> ArrayComponentType {
        match self {
            DescriptorTypeBasic::Byte => ArrayComponentType::Byte,
            DescriptorTypeBasic::Char => ArrayComponentType::Char,
            DescriptorTypeBasic::Double => ArrayComponentType::Double,
            DescriptorTypeBasic::Float => ArrayComponentType::Float,
            DescriptorTypeBasic::Int => ArrayComponentType::Int,
            DescriptorTypeBasic::Long => ArrayComponentType::Long,
            DescriptorTypeBasic::Class(x) => ArrayComponentType::Class(x),
            DescriptorTypeBasic::Short => ArrayComponentType::Short,
            DescriptorTypeBasic::Boolean => ArrayComponentType::Boolean,
        }
    }

    #[must_use]
    pub fn as_class_id(self) -> Option<ClassId> {
        if let DescriptorTypeBasic::Class(class_id) = self {
            Some(class_id)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_pretty_string(self, class_names: &ClassNames) -> String {
        match self {
            DescriptorTypeBasic::Class(id) => {
                if class_names.name_from_gcid(id).is_ok() {
                    let path = class_names.tpath(id);
                    path.to_owned()
                } else {
                    format!("[BadClassId #{}]", id.get())
                }
            }
            // All the primitive types can be handled with `name`
            _ => self.name().unwrap().to_owned(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorType {
    Basic(DescriptorTypeBasic),
    Array {
        level: NonZeroUsize,
        component: DescriptorTypeBasic,
    },
}
impl DescriptorType {
    pub fn from_class_file_desc(class_names: &mut ClassNames, desc: DescriptorTypeCF<'_>) -> Self {
        match desc {
            DescriptorTypeCF::Basic(x) => {
                Self::Basic(DescriptorTypeBasic::from_class_file_desc(x, class_names))
            }
            DescriptorTypeCF::Array { level, component } => Self::Array {
                level,
                component: DescriptorTypeBasic::from_class_file_desc(component, class_names),
            },
        }
    }

    #[must_use]
    /// Helper to construct a single level array of the type.
    /// type[]
    pub fn single_array(component: DescriptorTypeBasic) -> Self {
        Self::Array {
            level: NonZeroUsize::new(1).unwrap(),
            component,
        }
    }

    #[must_use]
    pub fn is_reference(&self) -> bool {
        matches!(
            self,
            DescriptorType::Array { .. } | DescriptorType::Basic(DescriptorTypeBasic::Class(_))
        )
    }

    pub fn as_class_id(&self, class_names: &mut ClassNames) -> Result<Option<ClassId>, BadIdError> {
        match self {
            DescriptorType::Basic(x) => Ok(x.as_class_id()),
            DescriptorType::Array { level, component } => {
                // TODO: Handling this conversion here is unfortunate, it shoudl be a part of
                // ClassNames
                let name = component.as_desc_iter(class_names)?;
                let class_name = std::iter::repeat(b"[" as &[u8])
                    .take(level.get())
                    .chain(name);
                let key = class_names.insert_key_from_iter_single(class_name);
                let id = class_names.insert_trusted_insert(key);
                Ok(Some(id))
            }
        }
    }

    #[must_use]
    pub fn as_pretty_string(&self, class_names: &ClassNames) -> String {
        match self {
            DescriptorType::Basic(basic) => basic.as_pretty_string(class_names),
            DescriptorType::Array { level, component } => {
                let mut result = component.as_pretty_string(class_names);
                for _ in 0..level.get() {
                    result += "[]";
                }
                result
            }
        }
    }
}

pub type ParametersContainer = SmallVec<[DescriptorType; 8]>;

#[derive(Debug, Clone, PartialEq)]
pub struct MethodDescriptor {
    parameters: ParametersContainer,
    /// None represents void
    return_type: Option<DescriptorType>,
}
impl MethodDescriptor {
    #[must_use]
    /// Construct a method descriptor that takes in the given parameters and potentially returns
    /// some type
    pub fn new(parameters: ParametersContainer, return_type: Option<DescriptorType>) -> Self {
        Self {
            parameters,
            return_type,
        }
    }

    #[must_use]
    /// Construct a [`MethodDescriptor`] that returns void
    pub fn new_void(parameters: impl Into<ParametersContainer>) -> Self {
        Self::new(parameters.into(), None)
    }

    #[must_use]
    /// Construct a [`MethodDescriptor`] that takes no parameters and returns void
    pub fn new_empty() -> Self {
        Self::new(ParametersContainer::new(), None)
    }

    #[must_use]
    /// Construct a [`MethodDescriptor`] that takes no parameters and returns some type
    pub fn new_ret(return_type: DescriptorType) -> Self {
        Self::new(ParametersContainer::new(), Some(return_type))
    }

    #[must_use]
    pub fn parameters(&self) -> &[DescriptorType] {
        self.parameters.as_slice()
    }

    #[must_use]
    pub fn return_type(&self) -> Option<&DescriptorType> {
        self.return_type.as_ref()
    }

    /// Returns `true` if the function accepts no parameters
    /// and returns void.
    #[must_use]
    pub fn is_nullary_void(&self) -> bool {
        self.parameters.is_empty() && self.return_type.is_none()
    }

    #[must_use]
    pub fn into_parameters_ret(self) -> (ParametersContainer, Option<DescriptorType>) {
        (self.parameters, self.return_type)
    }

    pub(crate) fn from_text_iter<'desc, 'names>(
        desc: &'desc [u8],
        class_names: &'names mut ClassNames,
    ) -> Result<MethodDescriptorParserIterator<'desc, 'names>, MethodDescriptorError> {
        MethodDescriptorParserIterator::new(desc, class_names)
    }

    pub fn from_text(
        desc: &[u8],
        class_names: &mut ClassNames,
    ) -> Result<Self, MethodDescriptorError> {
        let mut desc_iter = MethodDescriptorCF::parse_iter(desc)?;
        let mut parameters = SmallVec::new();
        #[allow(clippy::while_let_on_iterator)]
        while let Some(parameter) = desc_iter.next() {
            let parameter = parameter?;
            let parameter = DescriptorType::from_class_file_desc(class_names, parameter);
            parameters.push(parameter);
        }

        let return_type = desc_iter
            .finish_return_type()?
            .map(|x| DescriptorType::from_class_file_desc(class_names, x));
        Ok(Self {
            parameters,
            return_type,
        })
    }

    #[must_use]
    pub fn as_pretty_string(&self, class_names: &ClassNames) -> String {
        let mut result = "(".to_owned();
        for (i, parameter) in self.parameters.iter().enumerate() {
            result.push_str(parameter.as_pretty_string(class_names).as_str());
            if i + 1 < self.parameters.len() {
                result.push_str(", ");
            }
        }
        result.push_str(") -> ");

        if let Some(return_type) = &self.return_type {
            result.push_str(return_type.as_pretty_string(class_names).as_str());
        } else {
            result.push_str("void");
        }

        result
    }

    /// Checks if the descriptor is strictly equal to an unparsed descriptor
    /// This performs no casting checks or anything of the sort.
    pub fn is_equal_to_descriptor(
        &self,
        class_names: &mut ClassNames,
        desc: &[u8],
    ) -> Result<bool, MethodDescriptorError> {
        let mut iter = MethodDescriptor::from_text_iter(desc, class_names)?;
        // We can't use enumerate because we need the original iterator and there's no way to get it back out
        let mut i = 0;
        // A for loop would consume the iterator
        #[allow(clippy::while_let_on_iterator)]
        while let Some(parameter) = iter.next() {
            let parameter = parameter?;

            if let Some(self_parameter) = self.parameters.get(i) {
                if self_parameter != &parameter {
                    // One of the parameters was not equal, so it isn't the same
                    return Ok(false);
                }
            } else {
                // There was no entry at that index, so the descriptor had more parameters than self
                return Ok(false);
            }

            i += 1;
        }

        // We've iterated over all of the text-descriptor's types, but we still need to check if
        // we have any more types remaining that haven't been checked.
        // So we check if the next index would have been valid
        if i < self.parameters.len() {
            return Ok(false);
        }

        // If their return types are unequal then they can't be equal
        let return_type = iter.finish_return_type()?;
        if return_type != self.return_type {
            return Ok(false);
        }

        Ok(true)
    }
}

pub struct MethodDescriptorParserIterator<'desc, 'names> {
    class_names: &'names mut ClassNames,
    iter: MethodDescriptorParserIteratorCF<'desc>,
}
impl<'desc, 'names> MethodDescriptorParserIterator<'desc, 'names> {
    fn new(
        desc: &'desc [u8],
        class_names: &'names mut ClassNames,
    ) -> Result<MethodDescriptorParserIterator<'desc, 'names>, MethodDescriptorError> {
        let iter = MethodDescriptorCF::parse_iter(desc)?;
        Ok(MethodDescriptorParserIterator { class_names, iter })
    }

    pub fn finish_return_type(self) -> Result<Option<DescriptorType>, MethodDescriptorError> {
        self.iter
            .finish_return_type()
            .map(|x| x.map(|x| DescriptorType::from_class_file_desc(self.class_names, x)))
    }
}
impl<'desc, 'names> Iterator for MethodDescriptorParserIterator<'desc, 'names> {
    type Item = Result<DescriptorType, MethodDescriptorError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|x| x.map(|x| DescriptorType::from_class_file_desc(self.class_names, x)))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodOverride {
    /// The method that is overridden
    method_id: MethodId,
}
impl MethodOverride {
    pub(crate) fn new(method_id: MethodId) -> Self {
        Self { method_id }
    }
}

// Clippy's suggestion is less immediately clear
// This also has to be on the function itself since it doesn't seem to see if it is put before the
// if block
#[allow(clippy::nonminimal_bool)]
pub(crate) fn verify_method_access_flags(
    flags: MethodAccessFlags,
) -> Result<(), VerifyMethodError> {
    let is_public = flags.contains(MethodAccessFlags::PUBLIC);
    let is_protected = flags.contains(MethodAccessFlags::PROTECTED);
    let is_private = flags.contains(MethodAccessFlags::PRIVATE);

    // It can only have one of the bits set
    if (is_public && is_private) || (is_public && is_protected) || (is_private && is_protected) {
        return Err(VerifyMethodError::IncompatibleVisibilityModifiers);
    }

    // A method like <clinit> (a static init block) might not have any of these set, so we ignore
    // if they are all not set.

    Ok(())
}

/// Whether the method can override some super class method
pub(crate) fn can_method_override(flags: MethodAccessFlags) -> bool {
    // Private methods can't override
    // Static methods can't override in the same way that normal methods do.
    // They can shadow, though, but this is not an override.
    !(flags.contains(MethodAccessFlags::PRIVATE) || flags.contains(MethodAccessFlags::STATIC))
}
