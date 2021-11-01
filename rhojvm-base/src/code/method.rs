use std::num::NonZeroUsize;

use classfile_parser::{
    attribute_info::AttributeInfo,
    descriptor::{
        method::{MethodDescriptor as MethodDescriptorCF, MethodDescriptorError},
        DescriptorType as DescriptorTypeCF, DescriptorTypeBasic as DescriptorTypeBasicCF,
    },
    method_info::{MethodAccessFlags, MethodInfo},
    ClassAccessFlags,
};

use crate::{
    class::{ArrayComponentType, ClassFileData},
    id::{ClassId, MethodId},
    ClassNames, LoadMethodError, VerifyMethodError,
};

use super::CodeInfo;

// TODO: We could have a MethodAlias (so Method becomes an enum of the current Method and
// MethodAlias), which simply has the id of another method. This would then allow us to have
// a Command to look for duplicates, and to limit generation of duplicates.
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
    fn from_class_file_desc(desc: DescriptorTypeBasicCF<'_>, class_names: &mut ClassNames) -> Self {
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
    fn from_class_file_desc(desc: DescriptorTypeCF<'_>, class_names: &mut ClassNames) -> Self {
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
                .map(|x| DescriptorType::from_class_file_desc(x, class_names))
                .collect(),
            return_type: return_type.map(|x| DescriptorType::from_class_file_desc(x, class_names)),
        }
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
