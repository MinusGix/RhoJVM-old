use std::collections::HashMap;

use classfile_parser::{
    constant_info::Utf8Constant,
    constant_pool::ConstantPoolIndexRaw,
    method_info::{MethodAccessFlags, MethodInfoOpt},
};
use smallvec::{smallvec, SmallVec};

use crate::{
    class::{ClassFileInfo, ClassVariant},
    code::{
        method::{self, Method, MethodDescriptor, MethodOverride},
        op_ex::InstructionParseError,
        CodeInfo,
    },
    data::classes::does_extend_class,
    id::{ClassId, ExactMethodId, MethodId, MethodIndex, PackageId},
    package::Packages,
    util::{self, Cesu8String},
    StepError,
};

use super::{
    class_files::ClassFiles,
    class_names::ClassNames,
    classes::{load_descriptor_type, Classes},
};

#[derive(Debug)]
#[non_exhaustive]
pub enum LoadMethodError {
    /// There was no method at that id
    NonexistentMethod { id: ExactMethodId },
    /// There was no method with that name
    NonexistentMethodName {
        class_id: ClassId,
        name: Cesu8String,
    },
    /// The index to the name of the method was invalid
    InvalidMethodNameIndex {
        index: ConstantPoolIndexRaw<Utf8Constant>,
    },
    /// The index to the descriptor of the method was invalid
    InvalidDescriptorIndex {
        index: ConstantPoolIndexRaw<Utf8Constant>,
    },
    /// An error in parsing the method descriptor
    MethodDescriptorError(classfile_parser::descriptor::method::MethodDescriptorError),
}

#[derive(Debug)]
#[non_exhaustive]
pub enum LoadCodeError {
    InvalidCodeAttribute,
    InstructionParse(InstructionParseError),
    /// The method index was invalid, this could signify either a logical bug with this lib
    /// or the code using it.
    BadMethodIndex,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum VerifyCodeExceptionError {
    /// The indices were from larger to greater, which is not allowed.
    InverseOrder,
    /// There was no code at the start index
    InvalidStartIndex,
    /// There was no code at the end index
    InvalidEndIndex,
    /// There was no code at the handler index
    InvalidHandlerIndex,
    /// The const pool index for the catch type was invalid (zero is allowed)
    InvalidCatchTypeIndex,
    /// The const pool index for the class name was invalid
    InvalidCatchTypeNameIndex,
    /// The catch type did not extend Throwable, which is required
    NonThrowableCatchType,
    /// The const pool index for the method that InvokeSpecial invokes
    InvalidInvokeSpecialMethodIndex,
    /// The type of constant pool information at that position was unexpected
    /// This could theoretically mean a bug in the library
    InvalidInvokeSpecialInfo,
    /// The const pool index for the name_and_type constantinfo of the method was invalid
    InvalidInvokeSpecialMethodNameTypeIndex,
    /// The const pool index for name constantinfo of the method was invalid
    InvalidInvokeSpecialMethodNameIndex,
    /// There's illegal instructions used in the exception method / exception handler
    IllegalInstructions,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum VerifyMethodError {
    IncompatibleVisibilityModifiers,
}

#[derive(Debug, Default, Clone)]
pub struct Methods {
    map: HashMap<ExactMethodId, Method>,
}
impl Methods {
    #[must_use]
    pub fn new() -> Methods {
        Methods {
            map: HashMap::new(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[must_use]
    pub fn contains_key(&self, key: &ExactMethodId) -> bool {
        self.map.contains_key(key)
    }

    #[must_use]
    pub fn get(&self, key: &ExactMethodId) -> Option<&Method> {
        self.map.get(key)
    }

    #[must_use]
    pub fn get_mut(&mut self, key: &ExactMethodId) -> Option<&mut Method> {
        self.map.get_mut(key)
    }

    pub(crate) fn set_at(&mut self, key: ExactMethodId, val: Method) {
        if self.map.insert(key, val).is_some() {
            tracing::warn!("Duplicate setting for Methods with {:?}", key);
            debug_assert!(false);
        }
    }

    // TODO: Version that gets the class directly and the method's index

    /// If this returns `Ok(())` then it it assured to exist on this with the same id
    pub fn load_method_from_id(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        method_id: ExactMethodId,
    ) -> Result<(), StepError> {
        if self.contains_key(&method_id) {
            return Ok(());
        }

        let (class_id, method_index) = method_id.decompose();
        class_files.load_by_class_path_id(class_names, class_id)?;
        let class_file = class_files.get(&class_id).unwrap();

        let method = direct_load_method_from_index(class_names, class_file, method_index)?;
        self.set_at(method_id, method);
        Ok(())
    }

    pub fn load_method_from_index(
        &mut self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        method_index: MethodIndex,
    ) -> Result<(), StepError> {
        let method_id = ExactMethodId::unchecked_compose(class_file.id(), method_index);
        if self.contains_key(&method_id) {
            return Ok(());
        }

        let method = direct_load_method_from_index(class_names, class_file, method_index)?;
        self.set_at(method_id, method);
        Ok(())
    }

    pub fn load_method_from_desc(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        class_id: ClassId,
        name: &[u8],
        desc: &MethodDescriptor,
    ) -> Result<ExactMethodId, StepError> {
        class_files.load_by_class_path_id(class_names, class_id)?;
        let class_file = class_files.get(&class_id).unwrap();

        let (method_id, method_info) = method_id_from_desc(class_names, class_file, name, desc)?;

        if self.contains_key(&method_id) {
            return Ok(method_id);
        }

        let method = Method::new_from_info(method_id, class_file, class_names, method_info)?;

        self.set_at(method_id, method);
        Ok(method_id)
    }

    /// Load all methods from the classfile into methods
    /// This avoids filling the backing, but uses it if it is already filled
    pub fn load_all_methods_from(
        &mut self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
    ) -> Result<(), LoadMethodError> {
        let class_id = class_file.id();
        let methods_opt_iter = class_file.load_method_info_opt_iter_with_index();
        for (method_index, method_info) in methods_opt_iter {
            let method_id = ExactMethodId::unchecked_compose(class_id, method_index);
            let method = Method::new_from_info(method_id, class_file, class_names, method_info)?;
            self.set_at(method_id, method);
        }

        Ok(())
    }
}

pub fn direct_load_method_from_index(
    class_names: &mut ClassNames,
    class_file: &ClassFileInfo,
    method_index: MethodIndex,
) -> Result<Method, StepError> {
    let method_id = ExactMethodId::unchecked_compose(class_file.id(), method_index);
    let method = class_file
        .load_method_info_opt_by_index(method_index)
        .map_err(|_| LoadMethodError::NonexistentMethod { id: method_id })?;
    let method = Method::new_from_info(method_id, class_file, class_names, method)?;

    Ok(method)
}

fn method_id_from_desc<'a>(
    class_names: &mut ClassNames,
    class_file: &'a ClassFileInfo,
    name: &[u8],
    desc: &MethodDescriptor,
) -> Result<(ExactMethodId, MethodInfoOpt), StepError> {
    for (method_index, method_info) in class_file.load_method_info_opt_iter_with_index() {
        let name_index = method_info.name_index;
        let name_text = class_file.get_text_b(name_index);
        if name_text != Some(name) {
            continue;
        }

        let descriptor_index = method_info.descriptor_index;
        let descriptor_text = class_file.get_text_b(descriptor_index).ok_or(
            LoadMethodError::InvalidDescriptorIndex {
                index: descriptor_index,
            },
        )?;

        if desc
            .is_equal_to_descriptor(class_names, descriptor_text)
            .map_err(LoadMethodError::MethodDescriptorError)?
        {
            let method_id = ExactMethodId::unchecked_compose(class_file.id(), method_index);

            return Ok((method_id, method_info));
        }
    }

    Err(LoadMethodError::NonexistentMethodName {
        class_id: class_file.id(),
        name: util::Cesu8String(name.to_owned()),
    }
    .into())
}

pub fn direct_load_method_from_desc(
    class_names: &mut ClassNames,
    class_file: &ClassFileInfo,
    name: &[u8],
    desc: &MethodDescriptor,
) -> Result<Method, StepError> {
    let (method_id, method_info) = method_id_from_desc(class_names, class_file, name, desc)?;

    Ok(Method::new_from_info(
        method_id,
        class_file,
        class_names,
        method_info,
    )?)
}

pub fn load_method_descriptor_types(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    method: &Method,
) -> Result<(), StepError> {
    for parameter_type in method.descriptor().parameters().iter().copied() {
        load_descriptor_type(classes, class_names, class_files, packages, parameter_type)?;
    }

    if let Some(return_type) = method.descriptor().return_type().copied() {
        load_descriptor_type(classes, class_names, class_files, packages, return_type)?;
    }

    Ok(())
}

fn helper_get_overrided_method(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    super_class_file_id: ClassId,
    over_package: Option<PackageId>,
    over_method: &Method,
    over_method_name: Vec<u8>,
) -> Result<Option<MethodId>, StepError> {
    classes.load_class(class_names, class_files, packages, super_class_file_id)?;
    // We reget it, so that it does not believe we have `self` borrowed mutably
    let super_class = classes
        .get(&super_class_file_id)
        .ok_or(StepError::MissingLoadedValue(
            "helper_get_overrided_method : super_class",
        ))?;
    let super_class = if let ClassVariant::Class(super_class) = super_class {
        super_class
    } else {
        // TODO:
        eprintln!("Skipped trying to find overrides on an extended array class");
        return Ok(None);
    };

    // We don't need to parse every method for finding the override., at least not right now
    let super_class_file =
        class_files
            .get(&super_class_file_id)
            .ok_or(StepError::MissingLoadedValue(
                "helper_get_overrided_method : super_class_file",
            ))?;
    for (i, method) in super_class_file.load_method_info_opt_iter_with_index() {
        let flags = method.access_flags;
        let is_public = flags.contains(MethodAccessFlags::PUBLIC);
        let is_protected = flags.contains(MethodAccessFlags::PROTECTED);
        let is_private = flags.contains(MethodAccessFlags::PRIVATE);
        let is_final = flags.contains(MethodAccessFlags::FINAL);

        // TODO: https://docs.oracle.com/javase/specs/jls/se8/html/jls-8.html#jls-8.4.3
        // if the signature is a subsignature of the super class method
        // https://docs.oracle.com/javase/specs/jls/se8/html/jls-8.html#jls-8.4.2
        // which requires type erasure
        //  https://docs.oracle.com/javase/specs/jls/se8/html/jls-4.html#jls-4.6
        // might be able to avoid parsing their types since type erasure seems
        // more limited than casting to a base class, and so only need to know
        // if types are equivalent which can be done with package paths and typenames

        // We can access it because it is public and/or it is protected (and we are
        // inheriting from it), and it isn't private.
        let is_inherited_accessible = (is_public || is_protected) && !is_private;

        // Whether we are in the same package
        // TODO: I find this line confusing:
        // 'is marked neither ACC_PUBLIC nor ACC_PROTECTED nor ACC_PRIVATE and A
        // belongs to the same run-time package as C'
        // For now, I'll just ignore that and assume it is package accessible for any
        // access flags, but this might imply that it is a function with none of them
        // TODO: What is the definition of a package? We're being quite inexact
        // A class might be a package around a sub-class (defined inside of it)
        // and then there's subclasses for normal
        // TODO: is our assumption that no-package is not the same as no-package, right?
        let is_package_accessible = super_class
            .package
            .zip(over_package)
            .map_or(false, |(l, r)| l == r);

        let is_overridable = !is_final;

        if is_inherited_accessible || is_package_accessible {
            // TODO:
            // 'An instance method mC declared in class C overrides another instance method mA
            // declared in class A iff either mC is the same as mA, or all of the following are
            // true:' The part mentioning 'iff either mC is the same as mA' is confusing.
            // Does this mean if they are _literally_ the same, as in codewise too?
            // or does it mean if they are the same method (aka A == C)? That technically would
            // mean that a method overrides itself.
            // Or is there some in-depth equality of methods defined somewhere?

            let method_name = super_class_file.get_text_b(method.name_index).ok_or(
                LoadMethodError::InvalidDescriptorIndex {
                    index: method.name_index,
                },
            )?;

            // TODO: We currently pass in the over method name as a vec because it is typically
            // owned by the `Methods` which is passed in, and so would be a multiple mutable borrow
            // However, it would be nice to have some way of avoiding that alloc
            if method_name == over_method_name {
                // TODO: Don't do allocation for comparison. Either have a way to just directly
                // compare method descriptors with the parsed versions, or a streaming parser
                // for comparison without alloc
                let method_desc = super_class_file.get_text_b(method.descriptor_index).ok_or(
                    LoadMethodError::InvalidDescriptorIndex {
                        index: method.descriptor_index,
                    },
                )?;
                let method_desc = MethodDescriptor::from_text(method_desc, class_names)
                    .map_err(LoadMethodError::MethodDescriptorError)?;

                // TODO: Is there more complex rules for equivalent descriptors?
                if method_desc == over_method.descriptor {
                    // We wait to check if the method is overridable (!final) until here because
                    // if it _is_ final then we can't override *past* it to some super class
                    // that had a non-final version since we're extending the class with the
                    // final version.
                    if is_overridable {
                        return Ok(Some(MethodId::unchecked_compose(super_class.id, i)));
                    }
                }
            }
        }

        // Otherwise, ignore the method
    }

    // Now, just because we failed to find a method that matched doesn't mean we can stop now.
    // We have to check the *next* super class method, all the way up the chain to no super.

    if let Some(super_super_class_file_id) = super_class.super_class {
        // We could actually support this, but it is rough, and probably unneeded.
        debug_assert_ne!(
            super_super_class_file_id, super_class.id,
            "A class had its own super class be itself"
        );
        helper_get_overrided_method(
            class_names,
            class_files,
            classes,
            packages,
            super_super_class_file_id,
            over_package,
            over_method,
            over_method_name,
        )
    } else {
        // There was no method.
        Ok(None)
    }
}

pub fn init_method_overrides(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    method_id: ExactMethodId,
) -> Result<(), StepError> {
    methods.load_method_from_id(class_names, class_files, method_id)?;

    let (class_id, _) = method_id.decompose();
    // It should have both the class and method
    let class = classes.get(&class_id).unwrap();
    let class = if let ClassVariant::Class(class) = class {
        class
    } else {
        // TODO: There might be one override on them? But if we implement this well,
        // we probably don't need to compute overrides for them anyway.
        eprintln!("Skipped trying to find overrides for an array class");
        return Ok(());
    };
    let class_file = class_files.get(&class_id).unwrap();
    let package = class.package;

    let method = methods.get(&method_id).unwrap();

    // We have already collected the overrides.
    if method.overrides.is_some() {
        return Ok(());
    }

    let access_flags = method.access_flags;
    // Only some methods can override at all.
    if !method::can_method_override(access_flags) {
        return Ok(());
    }

    let overrides = {
        let method_name = class_file
            .get_text_b(method.name_index())
            .ok_or(StepError::MissingLoadedValue("method name.index"))?
            .to_owned();
        if let Some(super_class_file_id) = class.super_class {
            if let Some(overridden) = helper_get_overrided_method(
                class_names,
                class_files,
                classes,
                packages,
                super_class_file_id,
                package,
                method,
                method_name,
            )? {
                smallvec![MethodOverride::new(overridden)]
            } else {
                SmallVec::new()
            }
        } else {
            // There is no super class (so, in standard we must be Object), and so we don't have
            // to worry about a method overriding a super-class, since we don't have one and/or
            // we are the penultimate super-class.
            SmallVec::new()
        }
    };

    let method = methods
        .get_mut(&method_id)
        .ok_or(StepError::MissingLoadedValue(
            "init_method_overrides : method (post)",
        ))?;
    method.overrides = Some(overrides);

    Ok(())
}

pub fn verify_code_exceptions(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    method: &mut Method,
) -> Result<(), StepError> {
    fn get_class(
        class_files: &ClassFiles,
        method_id: ExactMethodId,
    ) -> Result<&ClassFileInfo, StepError> {
        let (class_id, _) = method_id.decompose();
        let class_file = class_files
            .get(&class_id)
            .ok_or(StepError::MissingLoadedValue(
                "verify_code_exceptions : class_file",
            ))?;
        Ok(class_file)
    }
    fn get_code(method: &Method) -> Result<&CodeInfo, StepError> {
        let code = method.code().ok_or(StepError::MissingLoadedValue(
            "verify_code_exceptions : method.code",
        ))?;
        Ok(code)
    }

    // TODO: What if it has code despite this
    if !method.should_have_code() {
        return Ok(());
    }

    method.load_code(class_files)?;

    let code = get_code(method)?;

    if code.exception_table().is_empty() {
        // There are no exceptions to verify
        return Ok(());
    }

    let throwable_id = class_names.gcid_from_bytes(b"java/lang/Throwable");

    let exception_table_len = code.exception_table().len();
    for exc_i in 0..exception_table_len {
        {
            let class_file = get_class(class_files, method.id())?;
            let code = get_code(method)?;
            debug_assert_eq!(code.exception_table().len(), exception_table_len);

            let exc = &code.exception_table()[exc_i];

            // TODO: option for whether this should be checked
            // If there is a catch type, then we check it
            // If there isn't, then it represents any exception and automatically passes
            // these checks
            if !exc.catch_type.is_zero() {
                let catch_type = class_file
                    .get_t(exc.catch_type)
                    .ok_or(VerifyCodeExceptionError::InvalidCatchTypeIndex)?;
                let catch_type_name = class_file
                    .get_text_b(catch_type.name_index)
                    .ok_or(VerifyCodeExceptionError::InvalidCatchTypeNameIndex)?;
                let catch_type_id = class_names.gcid_from_bytes(catch_type_name);

                if !does_extend_class(
                    class_names,
                    class_files,
                    classes,
                    catch_type_id,
                    throwable_id,
                )? {
                    return Err(VerifyCodeExceptionError::NonThrowableCatchType.into());
                }
            }
        }
        {
            // The above check for the class may have invalidated the references
            let class_file = get_class(class_files, method.id())?;
            let code = get_code(method)?;
            let exc = &code.exception_table()[exc_i];
            debug_assert_eq!(code.exception_table().len(), exception_table_len);

            code.instructions()
                .check_exception(class_file, method, exc)?;
        }
    }

    Ok(())
}
