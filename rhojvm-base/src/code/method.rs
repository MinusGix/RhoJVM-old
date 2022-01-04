use std::num::NonZeroUsize;

use classfile_parser::{
    attribute_info::AttributeInfo,
    descriptor::{
        method::{
            MethodDescriptor as MethodDescriptorCF, MethodDescriptorError,
            MethodDescriptorParserIterator as MethodDescriptorParserIteratorCF,
        },
        DescriptorType as DescriptorTypeCF, DescriptorTypeBasic as DescriptorTypeBasicCF,
    },
    method_info::{MethodAccessFlags, MethodInfo},
    ClassAccessFlags,
};

use crate::{
    class::{ArrayComponentType, ClassFileData},
    id::{ClassId, MethodId},
    BadIdError, ClassNames, LoadMethodError, VerifyMethodError,
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
    // TODO: Theoretically this can just be gotten by getting the data from the class file, which
    // avoids a load of allocations
    /// The name of the method
    pub(crate) name: String,
    /// Parameters and return type of the methods
    pub(crate) descriptor: MethodDescriptor,
    /// The access flags for the method
    pub(crate) access_flags: MethodAccessFlags,
    /// The methods that are overridden by this.
    /// `None` if it has not been initialized
    pub(crate) overrides: Option<Vec<MethodOverride>>,
    pub(crate) code: Option<CodeInfo>,
    /// Attributes may be removed at will as they are initialized
    pub(crate) attributes: Vec<AttributeInfo>,
}
impl Method {
    pub(crate) fn new(
        id: MethodId,
        name: String,
        descriptor: MethodDescriptor,
        access_flags: MethodAccessFlags,
        attributes: Vec<AttributeInfo>,
    ) -> Self {
        Self {
            id,
            name,
            descriptor,
            access_flags,
            overrides: None,
            code: None,
            attributes,
        }
    }

    pub(crate) fn new_from_info(
        id: MethodId,
        class_file: &ClassFileData,
        class_names: &mut ClassNames,
        method: &MethodInfo,
    ) -> Result<Self, LoadMethodError> {
        let method_name = class_file.get_text_t(method.name_index).ok_or(
            LoadMethodError::InvalidMethodNameIndex {
                index: method.name_index,
            },
        )?;
        Self::new_from_info_with_name(id, class_file, class_names, method, method_name.to_owned())
    }

    /// Construct the method with an already known name
    /// NOTE: This should _always_ be the same as the method's actual name.
    pub(crate) fn new_from_info_with_name(
        id: MethodId,
        class_file: &ClassFileData,
        class_names: &mut ClassNames,
        method: &MethodInfo,
        method_name: String,
    ) -> Result<Self, LoadMethodError> {
        debug_assert_eq!(
            class_file.get_text_t(method.name_index),
            Some(method_name.as_str())
        );

        let descriptor_text = class_file.get_text_t(method.descriptor_index).ok_or(
            LoadMethodError::InvalidDescriptorIndex {
                index: method.descriptor_index,
            },
        )?;
        let desc = MethodDescriptorCF::parse(descriptor_text)
            .map_err(LoadMethodError::MethodDescriptorError)?;
        let desc = MethodDescriptor::from_class_file_parser_md(desc, class_names);

        Ok(Method::new(
            id,
            method_name,
            desc,
            method.access_flags,
            method.attributes.clone(),
        ))
    }

    #[must_use]
    pub fn id(&self) -> MethodId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Whether or not it is an <init> function
    pub fn is_init(&self) -> bool {
        self.name() == "<init>"
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
    pub fn attributes(&self) -> &[AttributeInfo] {
        self.attributes.as_slice()
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
}

#[derive(Debug, Clone, PartialEq)]
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
    pub fn to_desc_string(&self, class_names: &mut ClassNames) -> Result<String, BadIdError> {
        match self {
            DescriptorTypeBasic::Byte => Ok("B".to_owned()),
            DescriptorTypeBasic::Char => Ok("C".to_owned()),
            DescriptorTypeBasic::Double => Ok("D".to_owned()),
            DescriptorTypeBasic::Float => Ok("F".to_owned()),
            DescriptorTypeBasic::Int => Ok("I".to_owned()),
            DescriptorTypeBasic::Long => Ok("J".to_owned()),
            DescriptorTypeBasic::Class(class_id) => {
                let name = class_names.name_from_gcid(*class_id)?;
                let path = name.path();
                if name.is_array() {
                    // If we have the id for an array then we just use the singular path it has
                    // because writing it as an object is incorrect.
                    Ok(path[0].clone())
                } else {
                    Ok(format!("L{path};", path = path.join("/")))
                }
            }
            DescriptorTypeBasic::Short => Ok("S".to_owned()),
            DescriptorTypeBasic::Boolean => Ok("Z".to_owned()),
        }
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
            DescriptorTypeBasicCF::ClassName(name) => {
                Self::Class(class_names.gcid_from_str(name.as_ref()))
            }
            DescriptorTypeBasicCF::Short => Self::Short,
            DescriptorTypeBasicCF::Boolean => Self::Boolean,
        }
    }

    pub(crate) fn name(&self) -> Option<&str> {
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

    pub(crate) fn access_flags(&self) -> Option<ClassAccessFlags> {
        match self {
            DescriptorTypeBasic::Class(_) => None,
            _ => Some(ClassAccessFlags::PUBLIC),
        }
    }

    pub(crate) fn as_array_component_type(&self) -> ArrayComponentType {
        match self {
            DescriptorTypeBasic::Byte => ArrayComponentType::Byte,
            DescriptorTypeBasic::Char => ArrayComponentType::Char,
            DescriptorTypeBasic::Double => ArrayComponentType::Double,
            DescriptorTypeBasic::Float => ArrayComponentType::Float,
            DescriptorTypeBasic::Int => ArrayComponentType::Int,
            DescriptorTypeBasic::Long => ArrayComponentType::Long,
            DescriptorTypeBasic::Class(x) => ArrayComponentType::Class(*x),
            DescriptorTypeBasic::Short => ArrayComponentType::Short,
            DescriptorTypeBasic::Boolean => ArrayComponentType::Boolean,
        }
    }

    pub fn as_class_id(&self) -> Option<ClassId> {
        if let DescriptorTypeBasic::Class(class_id) = self {
            Some(*class_id)
        } else {
            None
        }
    }

    pub fn as_pretty_string(&self, class_names: &ClassNames) -> String {
        match self {
            DescriptorTypeBasic::Class(id) => {
                if let Ok(name) = class_names.display_path_from_gcid(*id) {
                    format!("{}", name)
                } else {
                    format!("[BadClassId #{}]", *id)
                }
            }
            // All the primitive types can be handled with `name`
            _ => self.name().unwrap().to_owned(),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
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

    pub fn as_class_id(&self, class_names: &mut ClassNames) -> Result<Option<ClassId>, BadIdError> {
        match self {
            DescriptorType::Basic(x) => Ok(x.as_class_id()),
            DescriptorType::Array { level, component } => {
                // TODO: We could replace to_desc_string with something that returns an iterator
                // over T: AsRef<str>
                // TODO: This could also avoid extra string allocs by hashing the parts directly.
                let name = component.to_desc_string(class_names)?;
                let class_name = std::iter::repeat("[")
                    .take(level.get())
                    .chain([name.as_str()].into_iter());
                let id = class_names.gcid_from_iter_single(class_name);
                Ok(Some(id))
            }
        }
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct MethodDescriptor {
    parameters: Vec<DescriptorType>,
    /// None represents void
    return_type: Option<DescriptorType>,
}
impl MethodDescriptor {
    #[must_use]
    /// Construct a method descriptor that takes in the given parameters and potentially returns
    /// some type
    pub fn new(parameters: Vec<DescriptorType>, return_type: Option<DescriptorType>) -> Self {
        Self {
            parameters,
            return_type,
        }
    }

    #[must_use]
    /// Construct a [`MethodDescriptor`] that returns void
    pub fn new_void(parameters: Vec<DescriptorType>) -> Self {
        Self::new(parameters, None)
    }

    #[must_use]
    /// Construct a [`MethodDescriptor`] that takes no parameters and returns void
    pub fn new_empty() -> Self {
        Self::new(Vec::new(), None)
    }

    #[must_use]
    /// Construct a [`MethodDescriptor`] that takes no parameters and returns some type
    pub fn new_ret(return_type: DescriptorType) -> Self {
        Self::new(Vec::new(), Some(return_type))
    }

    #[must_use]
    pub fn parameters(&self) -> &[DescriptorType] {
        self.parameters.as_slice()
    }

    #[must_use]
    pub fn return_type(&self) -> Option<&DescriptorType> {
        self.return_type.as_ref()
    }

    pub fn into_parameters_ret(self) -> (Vec<DescriptorType>, Option<DescriptorType>) {
        (self.parameters, self.return_type)
    }

    pub(crate) fn from_text_iter<'desc, 'names>(
        desc: &'desc str,
        class_names: &'names mut ClassNames,
    ) -> Result<MethodDescriptorParserIterator<'desc, 'names>, MethodDescriptorError> {
        MethodDescriptorParserIterator::new(desc, class_names)
    }

    pub(crate) fn from_text(
        desc: &str,
        class_names: &mut ClassNames,
    ) -> Result<Self, MethodDescriptorError> {
        let desc = classfile_parser::descriptor::method::MethodDescriptor::parse(desc)?;
        Ok(MethodDescriptor::from_class_file_parser_md(
            desc,
            class_names,
        ))
    }

    pub(crate) fn from_class_file_parser_md(
        desc: MethodDescriptorCF,
        class_names: &mut ClassNames,
    ) -> Self {
        let MethodDescriptorCF {
            parameter_types,
            return_type,
        } = desc;
        Self {
            parameters: parameter_types
                .into_iter()
                .map(|x| DescriptorType::from_class_file_desc(class_names, x))
                .collect(),
            return_type: return_type.map(|x| DescriptorType::from_class_file_desc(class_names, x)),
        }
    }

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

    pub fn is_equal_to_descriptor(
        &self,
        class_names: &mut ClassNames,
        desc: &str,
    ) -> Result<bool, MethodDescriptorError> {
        let mut iter = MethodDescriptor::from_text_iter(desc, class_names)?;
        // We can't use enumerate because we need the original iterator and there's no way to get it back out
        let mut i = 0;
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
        desc: &'desc str,
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
