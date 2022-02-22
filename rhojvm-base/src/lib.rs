#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
// This would be nice to re-enable eventually, but not while in active dev
#![allow(clippy::missing_errors_doc)]
// Shadowing is nice.
#![allow(clippy::shadow_unrelated)]
// Not awful, but it highlights entire function.
#![allow(clippy::unnecessary_wraps)]
// Cool idea but highlights entire function and is too aggressive.
#![allow(clippy::option_if_let_else)]
// It would be nice to have this, but: active dev, it highlights entire function, and currently the
// code unwraps in a variety of cases where it knows it is valid.
#![allow(clippy::missing_panics_doc)]
// This is nice to have for cases where we might want to rely on it not returning anything.
#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::unused_self)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::too_many_lines)]
// The way this library is designed has many arguments. Grouping them together would be nice for
// readability, but it makes it harder to minimize dependnecies which has other knock-on effects..
#![allow(clippy::too_many_arguments)]
// This is nice to have, but it activates on cheaply constructible enumeration variants.
#![allow(clippy::or_fun_call)]

use std::{
    borrow::Cow,
    fs::File,
    hash::{Hash, Hasher},
    io::Read,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{self, AtomicU64},
};

use class::{
    ArrayClass, ArrayComponentType, Class, ClassFileData, ClassFileIndexError, ClassVariant,
};
use classfile_parser::{
    class_parser_opt,
    constant_info::{ClassConstant, Utf8Constant},
    constant_pool::ConstantPoolIndexRaw,
    method_info::{MethodAccessFlags, MethodInfoOpt},
    ClassAccessFlags,
};
use code::{
    method::{self, DescriptorType, DescriptorTypeBasic, Method, MethodDescriptor},
    op_ex::InstructionParseError,
    stack_map::StackMapError,
    types::{PrimitiveType, StackInfoError},
};
use id::{ClassId, MethodId, MethodIndex, PackageId};
use indexmap::{Equivalent, IndexMap};
use package::Packages;
use smallvec::{smallvec, SmallVec};
use tracing::{info, span, Level};

use crate::code::{method::MethodOverride, CodeInfo};

pub mod class;
pub mod code;
pub mod id;
pub mod package;
pub mod util;

// Note: Currently all of these errors use non_exhaustive, but in the future that may be removed
// on some if there is a belief that they are likely to be stable.

#[derive(Debug, Clone)]
pub struct BadIdError {
    pub id: ClassId,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum LoadClassFileError {
    /// The path given was empty
    EmptyPath,
    /// That class file has already been loaded.
    /// Note that the [`Command::LoadClassFile`] does not care if it already exists, but
    /// this helps be explicit about expectations in the code
    AlreadyExists,
    /// The file didn't exist with the relative path
    NonexistentFile(PathBuf),
    /// There was an error in reading the file
    ReadError(std::io::Error),
    /// There was an error in parsing the class file
    ClassFileParseError(String),
    /// There was a bad class file id
    BadId(BadIdError),
}

#[derive(Debug)]
#[non_exhaustive]
pub enum LoadClassError {
    BadId(BadIdError),
    LoadClassFile(LoadClassFileError),
    ClassFileIndex(ClassFileIndexError),
    /// An invalid index into the constant pool for an interface
    BadInterfaceIndex(ConstantPoolIndexRaw<ClassConstant>),
    /// An invalid index for an interface's name into the constant pool
    BadInterfaceNameIndex(ConstantPoolIndexRaw<Utf8Constant>),
}
impl From<ClassFileIndexError> for LoadClassError {
    fn from(err: ClassFileIndexError) -> Self {
        Self::ClassFileIndex(err)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum LoadMethodError {
    /// There was no method at that id
    NonexistentMethod { id: MethodId },
    /// There was no method with that name
    NonexistentMethodName {
        class_id: ClassId,
        name: Cow<'static, [u8]>,
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

#[derive(Debug)]
#[non_exhaustive]
pub enum StepError {
    Custom(Box<dyn std::error::Error>),
    LoadClassFile(LoadClassFileError),
    LoadClass(LoadClassError),
    LoadMethod(LoadMethodError),
    VerifyMethod(VerifyMethodError),
    LoadCode(LoadCodeError),
    VerifyCodeException(VerifyCodeExceptionError),
    StackMapError(StackMapError),
    StackInfoError(StackInfoError),
    DescriptorTypeError(classfile_parser::descriptor::DescriptorTypeError),
    /// Some code loaded a value and then tried accessing it but it was missing.
    /// This might be a sign that it shouldn't assume that, or a sign of a bug elsewhere
    /// that caused it to not load but also not reporting an error.
    MissingLoadedValue(&'static str),
    /// Expected a class that wasn't an array
    ExpectedNonArrayClass,
    /// There was a problem indexing into the class file
    ClassFileIndex(ClassFileIndexError),
    /// There was a bad class id that didn't have a name stored
    BadId(BadIdError),
    /// The type held by a descriptor was unexpected
    UnexpectedDescriptorType,
}
impl From<LoadClassFileError> for StepError {
    fn from(err: LoadClassFileError) -> Self {
        Self::LoadClassFile(err)
    }
}
impl From<LoadClassError> for StepError {
    fn from(err: LoadClassError) -> Self {
        Self::LoadClass(err)
    }
}
impl From<LoadMethodError> for StepError {
    fn from(err: LoadMethodError) -> Self {
        Self::LoadMethod(err)
    }
}
impl From<VerifyMethodError> for StepError {
    fn from(err: VerifyMethodError) -> Self {
        Self::VerifyMethod(err)
    }
}
impl From<LoadCodeError> for StepError {
    fn from(err: LoadCodeError) -> Self {
        Self::LoadCode(err)
    }
}
impl From<VerifyCodeExceptionError> for StepError {
    fn from(err: VerifyCodeExceptionError) -> Self {
        Self::VerifyCodeException(err)
    }
}
impl From<StackMapError> for StepError {
    fn from(err: StackMapError) -> Self {
        Self::StackMapError(err)
    }
}
impl From<StackInfoError> for StepError {
    fn from(err: StackInfoError) -> Self {
        Self::StackInfoError(err)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ClassDirectories {
    directories: Vec<PathBuf>,
}
impl ClassDirectories {
    pub fn add(&mut self, path: &Path) -> std::io::Result<()> {
        self.directories.push(path.canonicalize()?);
        Ok(())
    }

    #[must_use]
    pub fn load_class_file_with_rel_path(&self, rel_path: &Path) -> Option<(PathBuf, File)> {
        for class_dir in &self.directories {
            // TODO: is it remotely feasible to not allocate without notable extra fs calls?
            let mut full_path = class_dir.clone();
            full_path.push(rel_path);

            if let Ok(file) = File::open(&full_path) {
                return Some((full_path, file));
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Whether access flags should be verified or not.
    /// Note: This doesn't completely disable the feature, it just stops functions
    /// that do multiple verification steps for you (such as verifying the entirty of a method)
    /// from performing verify method access flags
    /// The user can still manually do such.
    pub verify_method_access_flags: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            verify_method_access_flags: true,
        }
    }
}

__make_map!(pub Classes<ClassId, ClassVariant>; access);
impl Classes {
    // FIXME: This doesn't force any verification
    /// The given array class must have valid and correct fields!
    pub fn register_array_class(&mut self, array_class: ArrayClass) {
        self.set_at(array_class.id(), ClassVariant::Array(array_class));
    }

    pub fn load_class(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        class_file_id: ClassId,
    ) -> Result<(), StepError> {
        if self.contains_key(&class_file_id) {
            // It was already loaded
            return Ok(());
        }

        let (_, class_info) = class_names
            .name_from_gcid(class_file_id)
            .map_err(StepError::BadId)?;
        info!("====> C{:?}", class_names.tpath(class_file_id));

        if !class_info.has_class_file() {
            // Just load the array class
            self.get_array_class(
                class_directories,
                class_names,
                class_files,
                packages,
                class_file_id,
            )?;
            return Ok(());
        }

        // Requires the class file to be loaded
        if !class_files.contains_key(&class_file_id) {
            class_files.load_by_class_path_id(class_directories, class_names, class_file_id)?;
        }

        let class_file = class_files.get(&class_file_id).unwrap();

        let this_class_name = class_file
            .get_this_class_name()
            .map_err(LoadClassError::ClassFileIndex)?;
        let super_class_id = class_file
            .get_super_class_id(class_names)
            .map_err(LoadClassError::ClassFileIndex)?;

        let package = util::access_path_initial_part(this_class_name);
        let package = package.map(|package| packages.slice_path_create_if_needed(package));

        let class = Class::new(
            class_file_id,
            super_class_id,
            package,
            class_file.access_flags(),
            class_file.methods_len(),
        );

        self.set_at(class_file_id, ClassVariant::Class(class));

        Ok(())
    }

    // TODO: We could maybe generate the id for these various arrays without string
    // allocations so that we can simply check if they exist cheaply
    pub fn load_array_of_instances(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        class_id: ClassId,
    ) -> Result<ClassId, StepError> {
        let component_type = ArrayComponentType::Class(class_id);

        let id = class_names
            .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), class_id)
            .map_err(StepError::BadId)?;
        if let Some(class) = self.get(&id) {
            // It was already loaded
            debug_assert!(matches!(class, ClassVariant::Array(_)));
            return Ok(id);
        }

        let (package, access_flags) = {
            // TODO: For normal classes, we only need to load the class file
            self.load_class(
                class_directories,
                class_names,
                class_files,
                packages,
                class_id,
            )?;
            let class = self.get(&class_id).unwrap();
            (class.package(), class.access_flags())
        };
        let array = ArrayClass {
            id,
            super_class: class_names.object_id(),
            component_type,
            access_flags,
            package,
        };
        self.register_array_class(array);
        Ok(id)
    }

    pub fn load_array_of_primitives(
        &mut self,
        class_names: &mut ClassNames,
        prim: PrimitiveType,
    ) -> Result<ClassId, StepError> {
        let component_type = ArrayComponentType::from(prim);

        let array_id = class_names.gcid_from_array_of_primitives(prim);
        if let Some(class) = self.get(&array_id) {
            // It was already loaded
            debug_assert!(matches!(class, ClassVariant::Array(_)));
            return Ok(array_id);
        }

        let array = ArrayClass::new_unchecked(
            array_id,
            component_type,
            class_names.object_id(),
            // Since all the types are primitive, we can simply use this
            ClassAccessFlags::PUBLIC,
            None,
        );
        self.register_array_class(array);
        Ok(array_id)
    }

    pub fn load_level_array_of_primitives(
        &mut self,
        class_names: &mut ClassNames,
        level: NonZeroUsize,
        prim: PrimitiveType,
    ) -> Result<ClassId, StepError> {
        let array_id = class_names.gcid_from_level_array_of_primitives(level, prim);
        if let Some(class) = self.get(&array_id) {
            // It was already loaded
            debug_assert!(matches!(class, ClassVariant::Array(_)));
            return Ok(array_id);
        }

        // If level > 1 then the component type isn't the above component type, but rather
        // another array.
        // We don't bother registering it, only registering the name
        let component_type =
            if let Some(level) = level.get().checked_sub(1).and_then(NonZeroUsize::new) {
                let component_id = class_names.gcid_from_level_array_of_primitives(level, prim);
                ArrayComponentType::Class(component_id)
            } else {
                prim.into()
            };

        let array = ArrayClass::new_unchecked(
            array_id,
            component_type,
            class_names.object_id(),
            ClassAccessFlags::PUBLIC,
            None,
        );
        self.register_array_class(array);

        Ok(array_id)
    }

    pub fn load_level_array_of_desc_type_basic(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        level: NonZeroUsize,
        component: DescriptorTypeBasic,
    ) -> Result<ClassId, StepError> {
        let array_id = class_names
            .gcid_from_level_array_of_desc_type_basic(level, component)
            .map_err(StepError::BadId)?;
        if let Some(class) = self.get(&array_id) {
            // It was already loaded
            debug_assert!(matches!(class, ClassVariant::Array(_)));
            return Ok(array_id);
        }

        let component_id = load_basic_descriptor_type(
            self,
            class_directories,
            class_names,
            class_files,
            packages,
            &component,
        )?;

        let (package, access_flags) = if let Some(component_id) = component_id {
            // TODO: For normal classes, we only need to load the class file
            self.load_class(
                class_directories,
                class_names,
                class_files,
                packages,
                component_id,
            )?;
            let class = self.get(&component_id).unwrap();
            (class.package(), class.access_flags())
        } else {
            // These methods only return none if it was a class, but if it was then it would
            // be in the other branch
            (None, component.access_flags().unwrap())
        };

        // If level > 1 then the component type isn't the above component type, but rather
        // another array.
        // We don't bother registering it, only registering the name
        let component_type =
            if let Some(level) = level.get().checked_sub(1).and_then(NonZeroUsize::new) {
                let component_id = class_names
                    .gcid_from_level_array_of_desc_type_basic(level, component)
                    .map_err(StepError::BadId)?;
                ArrayComponentType::Class(component_id)
            } else {
                component.as_array_component_type()
            };

        let array = ArrayClass {
            id: array_id,
            super_class: class_names.object_id(),
            component_type,
            access_flags,
            package,
        };
        self.register_array_class(array);

        Ok(array_id)
    }

    pub fn load_level_array_of_class_id(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        level: NonZeroUsize,
        class_id: ClassId,
    ) -> Result<ClassId, StepError> {
        // TODO: Inline this so that we do slightly less work
        self.load_level_array_of_desc_type_basic(
            class_directories,
            class_names,
            class_files,
            packages,
            level,
            DescriptorTypeBasic::Class(class_id),
        )
    }

    /// Returns the [`ArrayClass`] if it is an array
    /// This should be used rather than loading the class itself, because this
    /// avoids loading classes that it doesn't need to.
    pub fn get_array_class(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        class_id: ClassId,
    ) -> Result<Option<&ArrayClass>, StepError> {
        use classfile_parser::descriptor::DescriptorType as DescriptorTypeCF;

        // This weird contains_key then unwrap get is to avoid unpleasant borrow checker errors
        if self.contains_key(&class_id) {
            return Ok(self.get(&class_id).unwrap().as_array());
        } else if class_files.get(&class_id).is_some() {
            return Ok(None);
        }

        // Otherwise, we load the class, if it has a classname.
        let (class_name, class_info) = class_names
            .name_from_gcid(class_id)
            .map_err(StepError::BadId)?;

        if !class_info.is_array() {
            // It isn't an array, but that's fine.
            return Ok(None);
        }

        let descriptor: DescriptorTypeCF<'static> = {
            // TODO: Return an error if this doesn't exist, but if it does not then that is sign
            // of an internal bug
            let (descriptor, remaining) = DescriptorTypeCF::parse(class_name.get())
                .map_err(StepError::DescriptorTypeError)?;
            // TODO: This should actually be a runtime error
            assert!(remaining.is_empty());
            // TODO: We shouldn't have to potentially allocate.
            descriptor.to_owned()
        };
        if let DescriptorTypeCF::Array { level, component } = descriptor {
            let component = DescriptorTypeBasic::from_class_file_desc(component, class_names);
            let id = self.load_level_array_of_desc_type_basic(
                class_directories,
                class_names,
                class_files,
                packages,
                level,
                component,
            )?;
            debug_assert_eq!(id, class_id);

            // TODO: Better error handling than unwrap
            let array = self.get(&id).unwrap().as_array().unwrap();
            Ok(Some(array))
        } else {
            // TODO: This is likely indicative of an internal error since name parsing thought this was an array!
            Err(StepError::UnexpectedDescriptorType)
        }
    }

    /// Note: This specifically checks if it is a super class, if they are equal it returns false
    pub fn is_super_class(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        class_id: ClassId,
        maybe_super_class_id: ClassId,
    ) -> Result<bool, StepError> {
        let object_id = class_names.object_id();

        if class_id == maybe_super_class_id {
            return Ok(false);
        }

        class_files.load_by_class_path_id(class_directories, class_names, class_id)?;

        // If this is an array, then it only extends the given class if it is java.lang.Object
        if let Some(_array_class) = self.get_array_class(
            class_directories,
            class_names,
            class_files,
            packages,
            class_id,
        )? {
            // Arrays only extend object
            return Ok(maybe_super_class_id == object_id);
        }

        // TODO: We could do a bit of optimization for if the class file was unloaded but the class
        // still existed
        // Load the class file, because we need the super id
        let mut current_class_id = class_id;
        loop {
            class_files.load_by_class_path_id(class_directories, class_names, current_class_id)?;
            let class_file = class_files.get(&current_class_id).unwrap();

            if let Some(super_id) = class_file
                .get_super_class_id(class_names)
                .map_err(StepError::ClassFileIndex)?
            {
                if super_id == maybe_super_class_id {
                    return Ok(true);
                }

                current_class_id = super_id;
            } else {
                break;
            }
        }

        Ok(false)
    }

    pub fn implements_interface(
        &self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        class_id: ClassId,
        impl_interface_id: ClassId,
    ) -> Result<bool, StepError> {
        // Special handling for arrays
        if class_names.is_array(class_id).map_err(StepError::BadId)? {
            let interfaces = ArrayClass::get_interface_names();
            for interface_name in interfaces {
                let id = class_names.gcid_from_bytes(interface_name);
                if impl_interface_id == id {
                    return Ok(true);
                }
            }

            return Ok(false);
        }

        let mut current_class_id = Some(class_id);

        while let Some(current_id) = current_class_id {
            let interfaces = {
                class_files.load_by_class_path_id(class_directories, class_names, current_id)?;
                let class_file = class_files.get(&current_id).unwrap();

                // Get all the interfaces. This is collected to a vec because we will invalidate the
                // class file reference
                let interfaces = class_file
                    .interfaces_indices_iter()
                    .collect::<SmallVec<[_; 8]>>();

                // Check all the topmost indices first
                for interface_index in interfaces.iter().copied() {
                    let interface_constant = class_file
                        .get_t(interface_index)
                        .ok_or(LoadClassError::BadInterfaceIndex(interface_index))?;
                    let interface_name =
                        class_file.get_text_b(interface_constant.name_index).ok_or(
                            LoadClassError::BadInterfaceNameIndex(interface_constant.name_index),
                        )?;
                    let interface_id = class_names.gcid_from_bytes(interface_name);

                    if interface_id == impl_interface_id {
                        return Ok(true);
                    }
                }

                interfaces
            };

            // Check if any of the interfaces implement it
            // This is done after the topmost interfaces are checked so that it makes those calls cheaper
            for interface_index in interfaces.iter().copied() {
                // Sadly, code can autocast an interface down to an interface that it extends
                // Ex: A extends B, B extends C
                // we can cast A down to C
                // The problem with this is that it requires loading every interface's class file..

                // We can't trust that the class file is still loaded.
                class_files.load_by_class_path_id(class_directories, class_names, current_id)?;
                let class_file = class_files.get(&current_id).unwrap();

                let interface_constant = class_file
                    .get_t(interface_index)
                    .ok_or(LoadClassError::BadInterfaceIndex(interface_index))?;
                let interface_name = class_file.get_text_b(interface_constant.name_index).ok_or(
                    LoadClassError::BadInterfaceNameIndex(interface_constant.name_index),
                )?;
                let interface_id = class_names.gcid_from_bytes(interface_name);

                if self.implements_interface(
                    class_directories,
                    class_names,
                    class_files,
                    interface_id,
                    impl_interface_id,
                )? {
                    return Ok(true);
                }
            }

            class_files.load_by_class_path_id(class_directories, class_names, current_id)?;
            let class_file = class_files.get(&current_id).unwrap();

            current_class_id = class_file
                .get_super_class_id(class_names)
                .map_err(StepError::ClassFileIndex)?;
        }

        Ok(false)
    }

    /// Checks if `class_id` is an array and can be downcasted to `target_id` (if it is an array)
    /// Ex: `java.lang.String[]` -> `Object[]`
    /// Note that this does not return true if they are of the same exact type
    /// That is because it is easy to determine from their class ids
    pub fn is_castable_array(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        class_id: ClassId,
        target_id: ClassId,
    ) -> Result<bool, StepError> {
        let class_array = if let Some(class_array) = self.get_array_class(
            class_directories,
            class_names,
            class_files,
            packages,
            class_id,
        )? {
            class_array
        } else {
            // It wasn't an array
            return Ok(false);
        };
        let class_elem = class_array.component_type();

        let target_array = if let Some(target_array) = self.get_array_class(
            class_directories,
            class_names,
            class_files,
            packages,
            target_id,
        )? {
            target_array
        } else {
            // It wasn't an array
            return Ok(false);
        };
        let target_elem = target_array.component_type();

        // If it isn't a class id then this would be comparison of primitive arrays which
        // can just be done by comparing the ids

        let class_elem_id = if let Some(class_elem_id) = class_elem.into_class_id() {
            class_elem_id
        } else {
            return Ok(false);
        };

        let target_elem_id = if let Some(target_elem_id) = target_elem.into_class_id() {
            target_elem_id
        } else {
            return Ok(false);
        };

        // if it can be cast down because it extends it (B[] -> A[])
        // if it can be cast down because target elem is an interface (A[] -> Cloneable[])
        // or if it can be cast down because it holds a castable array (B[][] -> A[][])
        Ok(self.is_super_class(
            class_directories,
            class_names,
            class_files,
            packages,
            class_elem_id,
            target_elem_id,
        )? || self.implements_interface(
            class_directories,
            class_names,
            class_files,
            class_elem_id,
            target_elem_id,
        )? || self.is_castable_array(
            class_directories,
            class_names,
            class_files,
            packages,
            class_elem_id,
            target_elem_id,
        )?)
    }
}
__make_map!(typical pub Methods<MethodId, Method>; access);
impl Methods {
    // TODO: Version that gets the class directly and the method's index

    /// If this returns `Ok(())` then it it assured to exist on this with the same id
    pub fn load_method_from_id(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        method_id: MethodId,
    ) -> Result<(), StepError> {
        if self.contains_key(&method_id) {
            return Ok(());
        }

        let (class_id, method_index) = method_id.decompose();
        class_files.load_by_class_path_id(class_directories, class_names, class_id)?;
        let class_file = class_files.get(&class_id).unwrap();

        let method = direct_load_method_from_index(class_names, class_file, method_index)?;
        self.set_at(method_id, method);
        Ok(())
    }

    pub fn load_method_from_index(
        &mut self,
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
        method_index: MethodIndex,
    ) -> Result<(), StepError> {
        let method_id = MethodId::unchecked_compose(class_file.id(), method_index);
        if self.contains_key(&method_id) {
            return Ok(());
        }

        let method = direct_load_method_from_index(class_names, class_file, method_index)?;
        self.set_at(method_id, method);
        Ok(())
    }

    pub fn load_method_from_desc(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        class_id: ClassId,
        name: &[u8],
        desc: &MethodDescriptor,
    ) -> Result<MethodId, StepError> {
        class_files.load_by_class_path_id(class_directories, class_names, class_id)?;
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
        class_file: &ClassFileData,
    ) -> Result<(), LoadMethodError> {
        let class_id = class_file.id();
        let methods_opt_iter = class_file.load_method_info_opt_iter_with_index();
        for (method_index, method_info) in methods_opt_iter {
            let method_id = MethodId::unchecked_compose(class_id, method_index);
            let method = Method::new_from_info(method_id, class_file, class_names, method_info)?;
            self.set_at(method_id, method);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
enum InternalKind {
    Array,
}
impl InternalKind {
    fn from_slice<T: AsRef<str>>(class_path: &[T]) -> Option<InternalKind> {
        class_path
            .get(0)
            .map(AsRef::as_ref)
            .and_then(InternalKind::from_str)
    }

    fn from_iter<'a>(mut class_path: impl Iterator<Item = &'a str>) -> Option<InternalKind> {
        class_path.next().and_then(InternalKind::from_str)
    }

    fn from_bytes(class_path: &[u8]) -> Option<InternalKind> {
        if id::is_array_class_bytes(class_path) {
            Some(InternalKind::Array)
        } else {
            None
        }
    }

    fn from_raw_class_name(class_path: RawClassNameSlice<'_>) -> Option<InternalKind> {
        Self::from_bytes(class_path.0)
    }

    fn from_str(class_path: &str) -> Option<InternalKind> {
        if id::is_array_class(class_path) {
            Some(InternalKind::Array)
        } else {
            None
        }
    }

    fn has_class_file(&self) -> bool {
        false
    }
}

/// An insert into [`ClassNames`] that is trusted, aka it has all the right values
/// and is computed to be inserted when we have issues getting borrowing right.
/// The variants are private
pub(crate) enum TrustedClassNameInsert {
    /// We already have it
    Id(ClassId),
    /// It needs to be inserted
    Data {
        class_name: RawClassName,
        kind: Option<InternalKind>,
    },
}

#[derive(Debug, Clone)]
pub struct RawClassName(pub Vec<u8>);
impl RawClassName {
    #[must_use]
    pub fn get(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn as_slice(&self) -> RawClassNameSlice<'_> {
        RawClassNameSlice(self.0.as_slice())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl Eq for RawClassName {}
impl PartialEq for RawClassName {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Hash for RawClassName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct RawClassNameSlice<'a>(&'a [u8]);
impl<'a> RawClassNameSlice<'a> {
    #[must_use]
    pub fn get(&self) -> &'a [u8] {
        self.0
    }

    #[must_use]
    pub fn to_owned(&self) -> RawClassName {
        RawClassName(self.0.to_owned())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl<'a> Equivalent<RawClassName> for RawClassNameSlice<'a> {
    fn equivalent(&self, key: &RawClassName) -> bool {
        self.0 == key.0
    }
}
impl<'a> Hash for RawClassNameSlice<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // This mimics normal slice hashing, but we explicitly decide how we hash slices
        // This is because in our iterator version, we can't rely on the hash_slice
        // iterating over each piece individually, so
        // [0, 1, 2, 3] might not be the same as hashing [0, 1] and then [2, 3]
        self.0.len().hash(state);
        for piece in self.0 {
            piece.hash(state);
        }
    }
}

/// Used when you have an iterator over slices of bytes which form a single [`RawClassName`]
/// when considered together.
/// This does not insert `/`
#[derive(Clone)]
pub struct RawClassNameBuilderator<I> {
    iter: I,
    length: usize,
}
impl<I> RawClassNameBuilderator<I> {
    pub fn new_single<'a>(iter: I) -> RawClassNameBuilderator<I>
    where
        I: Iterator<Item = &'a [u8]> + Clone,
    {
        RawClassNameBuilderator {
            iter: iter.clone(),
            length: iter.fold(0, |acc, x| acc + x.len()),
        }
    }

    pub fn new_split<'a, J: Iterator<Item = &'a [u8]> + Clone>(
        iter: J,
    ) -> RawClassNameBuilderator<impl Iterator<Item = &'a [u8]> + Clone> {
        let iter = itertools::intersperse(iter, b"/");
        RawClassNameBuilderator::new_single(iter)
    }
}
impl<'a, I: Iterator<Item = &'a [u8]> + Clone> RawClassNameBuilderator<I> {
    /// Compute the kind that this would be
    pub(crate) fn internal_kind(&self) -> Option<InternalKind> {
        self.iter.clone().next().and_then(InternalKind::from_bytes)
    }

    pub(crate) fn into_raw_class_name(self) -> RawClassName {
        RawClassName(self.iter.flatten().copied().collect::<Vec<u8>>())
    }
}
impl<'a, I: Iterator<Item = &'a [u8]> + Clone> Hash for RawClassNameBuilderator<I> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // This mimics slice hashing, as done in RawClassNameSlice
        self.length.hash(state);
        for part in self.iter.clone() {
            for piece in part {
                piece.hash(state);
            }
        }
    }
}
impl<'a, I: Iterator<Item = &'a [u8]> + Clone> Equivalent<RawClassName>
    for RawClassNameBuilderator<I>
{
    fn equivalent(&self, key: &RawClassName) -> bool {
        // If they aren't of the same length t hen they're certainly not equivalent
        if self.length != key.get().len() {
            return false;
        }

        // Their length is known to be equivalent due to the earlier check
        let iter = key
            .get()
            .iter()
            .copied()
            .zip(self.iter.clone().flatten().copied());
        for (p1, p2) in iter {
            if p1 != p2 {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Clone)]
pub struct ClassNameInfo {
    kind: Option<InternalKind>,
    id: ClassId,
}
impl ClassNameInfo {
    #[must_use]
    pub fn has_class_file(&self) -> bool {
        if let Some(kind) = &self.kind {
            kind.has_class_file()
        } else {
            true
        }
    }

    #[must_use]
    pub fn is_array(&self) -> bool {
        matches!(self.kind, Some(InternalKind::Array))
    }
}

#[derive(Debug)]
pub struct ClassNames {
    next_id: AtomicU64,
    names: IndexMap<RawClassName, ClassNameInfo>,
}
impl ClassNames {
    #[must_use]
    pub fn new() -> Self {
        let mut class_names = ClassNames {
            next_id: AtomicU64::new(0),
            // TODO: We could probably choose a better and more accurate default
            // For a basic program, it might fit under this limit
            names: IndexMap::with_capacity(32),
        };

        // Reserve the first id, 0, so it is always for Object
        class_names.gcid_from_bytes(b"java/lang/Object");

        class_names
    }

    /// Construct a new unique id
    fn get_new_id(&mut self) -> ClassId {
        // Based on https://en.cppreference.com/w/cpp/atomic/memory_order in the Relaxed ordering
        // section, Relaxed ordering should work good for a counter that is only incrementing.
        ClassId::new_unchecked(self.next_id.fetch_add(1, atomic::Ordering::Relaxed))
    }

    /// Get the id of `b"java/lang/Object"`. Cached.
    #[must_use]
    pub fn object_id(&self) -> ClassId {
        ClassId::new_unchecked(0)
    }

    /// Check if the given id is for an array
    pub fn is_array(&self, id: ClassId) -> Result<bool, BadIdError> {
        self.name_from_gcid(id).map(|x| x.1.is_array())
    }

    /// Get the name and class info for a given id
    pub fn name_from_gcid(
        &self,
        id: ClassId,
    ) -> Result<(RawClassNameSlice<'_>, &ClassNameInfo), BadIdError> {
        // TODO: Can this be done better?
        self.names
            .iter()
            .find(|(_, info)| info.id == id)
            .map(|(data, info)| (data.as_slice(), info))
            .ok_or_else(|| {
                debug_assert!(false, "name_from_gcid: Got a bad id {:?}", id);
                BadIdError { id }
            })
    }

    pub fn gcid_from_bytes(&mut self, class_path: &[u8]) -> ClassId {
        let class_path = RawClassNameSlice(class_path);
        let kind = InternalKind::from_raw_class_name(class_path);

        if let Some(entry) = self.names.get(&class_path) {
            return entry.id;
        }

        let id = self.get_new_id();
        self.names
            .insert(class_path.to_owned(), ClassNameInfo { kind, id });
        id
    }

    pub fn gcid_from_vec(&mut self, class_path: Vec<u8>) -> ClassId {
        let class_path = RawClassName(class_path);
        let kind = InternalKind::from_raw_class_name(class_path.as_slice());

        if let Some(entry) = self.names.get(&class_path) {
            return entry.id;
        }

        let id = self.get_new_id();
        self.names.insert(class_path, ClassNameInfo { kind, id });
        id
    }

    pub fn gcid_from_cow(&mut self, class_path: Cow<[u8]>) -> ClassId {
        let kind = InternalKind::from_bytes(&class_path);

        let class_name = RawClassNameSlice(class_path.as_ref());
        if let Some(entry) = self.names.get(&class_name) {
            return entry.id;
        }

        let id = self.get_new_id();
        self.names.insert(
            RawClassName(class_path.into_owned()),
            ClassNameInfo { kind, id },
        );
        id
    }

    pub fn gcid_from_iter_bytes<'a, I: Iterator<Item = &'a [u8]> + Clone>(
        &mut self,
        class_path: I,
    ) -> ClassId {
        let class_path = RawClassNameBuilderator::<I>::new_split(class_path);
        let kind = class_path.internal_kind();

        if let Some(entry) = self.names.get(&class_path) {
            return entry.id;
        }

        let id = self.get_new_id();
        self.names
            .insert(class_path.into_raw_class_name(), ClassNameInfo { kind, id });
        id
    }

    pub fn gcid_from_array_of_primitives(&mut self, prim: PrimitiveType) -> ClassId {
        let prefix = prim.as_desc_prefix();
        let class_path = [b"[", prefix];
        let class_path = RawClassNameBuilderator::new_single(class_path.into_iter());

        if let Some(entry) = self.names.get(&class_path) {
            return entry.id;
        }

        let id = self.get_new_id();
        let class_path = class_path.into_raw_class_name();
        self.names.insert(
            class_path,
            ClassNameInfo {
                // We already know it is an array
                kind: Some(InternalKind::Array),
                id,
            },
        );

        id
    }

    pub fn gcid_from_level_array_of_primitives(
        &mut self,
        level: NonZeroUsize,
        prim: PrimitiveType,
    ) -> ClassId {
        let prefix = prim.as_desc_prefix();
        let class_path = std::iter::repeat(b"[" as &[u8]).take(level.get());
        let class_path = class_path.chain([prefix]);
        let class_path = RawClassNameBuilderator::new_single(class_path);

        if let Some(entry) = self.names.get(&class_path) {
            return entry.id;
        }

        let id = self.get_new_id();
        let class_path = class_path.into_raw_class_name();
        self.names.insert(
            class_path,
            ClassNameInfo {
                kind: Some(InternalKind::Array),
                id,
            },
        );

        id
    }

    pub fn gcid_from_level_array_of_class_id(
        &mut self,
        level: NonZeroUsize,
        class_id: ClassId,
    ) -> Result<ClassId, BadIdError> {
        let (class_name, class_info) = self.name_from_gcid(class_id)?;

        let first_iter = std::iter::repeat(b"[" as &[u8]).take(level.get());

        // Different branches because the iterator will have a different type
        let class_path = if class_info.is_array() {
            // [[{classname} and the like
            let class_path = first_iter.chain([class_name.get()]);
            let class_path = RawClassNameBuilderator::new_single(class_path);

            // Check if it already exists
            if let Some(entry) = self.names.get(&class_path) {
                return Ok(entry.id);
            }

            class_path.into_raw_class_name()
        } else {
            // L{classname};
            let class_path = first_iter.chain([b"L", class_name.get(), b";"]);
            let class_path = RawClassNameBuilderator::new_single(class_path);

            // Check if it already exists
            if let Some(entry) = self.names.get(&class_path) {
                return Ok(entry.id);
            }

            class_path.into_raw_class_name()
        };

        // If we got here then it doesn't already exist
        let id = self.get_new_id();
        self.names.insert(
            class_path,
            ClassNameInfo {
                kind: Some(InternalKind::Array),
                id,
            },
        );

        Ok(id)
    }

    pub fn gcid_from_level_array_of_desc_type_basic(
        &mut self,
        level: NonZeroUsize,
        component: DescriptorTypeBasic,
    ) -> Result<ClassId, BadIdError> {
        let name_iter = component.as_desc_iter(self)?;
        let class_path = std::iter::repeat(b"[" as &[u8])
            .take(level.get())
            .chain(name_iter);
        let class_path = RawClassNameBuilderator::new_single(class_path);

        if let Some(entry) = self.names.get(&class_path) {
            return Ok(entry.id);
        }

        let class_path = class_path.into_raw_class_name();
        let id = self.get_new_id();
        self.names.insert(
            class_path,
            ClassNameInfo {
                kind: Some(InternalKind::Array),
                id,
            },
        );

        Ok(id)
    }

    pub(crate) fn insert_key_from_iter_single<'a>(
        &self,
        class_path: impl Iterator<Item = &'a [u8]> + Clone,
    ) -> TrustedClassNameInsert {
        let class_path = RawClassNameBuilderator::new_single(class_path);
        if let Some(entry) = self.names.get(&class_path) {
            TrustedClassNameInsert::Id(entry.id)
        } else {
            let kind = class_path.internal_kind();
            TrustedClassNameInsert::Data {
                class_name: class_path.into_raw_class_name(),
                kind,
            }
        }
    }

    pub(crate) fn insert_trusted_insert(&mut self, insert: TrustedClassNameInsert) -> ClassId {
        match insert {
            TrustedClassNameInsert::Id(id) => id,
            TrustedClassNameInsert::Data { class_name, kind } => {
                let id = self.get_new_id();
                self.names.insert(class_name, ClassNameInfo { kind, id });
                id
            }
        }
    }

    /// Get the information in a nice representation for logging
    /// The output of this function is not guaranteed
    #[must_use]
    pub fn tpath(&self, id: ClassId) -> &str {
        self.name_from_gcid(id)
            .map(|x| x.0)
            .map(|x| std::str::from_utf8(x.0))
            // It is fine for it to be invalid utf8, but at the current moment we don't bother
            // converting it
            .unwrap_or(Ok("[UNKNOWN CLASS NAME]"))
            .unwrap_or("[INVALID UTF8]")
    }
}

impl Default for ClassNames {
    fn default() -> Self {
        Self::new()
    }
}

__make_map!(pub ClassFiles<ClassId, ClassFileData>; access);
impl ClassFiles {
    /// This is primarily for the JVM impl to load classes from user input
    pub fn load_by_class_path_slice<T: AsRef<str>>(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_path: &[T],
    ) -> Result<ClassId, LoadClassFileError> {
        if class_path.is_empty() {
            return Err(LoadClassFileError::EmptyPath);
        }

        // TODO: This is probably not accurate for more complex utf8
        let class_file_id = class_names
            .gcid_from_iter_bytes(class_path.iter().map(AsRef::as_ref).map(str::as_bytes));
        let (class_file_name, class_file_info) = class_names.name_from_gcid(class_file_id).unwrap();
        debug_assert!(!class_file_name.is_empty());
        if !class_file_info.has_class_file() && self.contains_key(&class_file_id) {
            return Ok(class_file_id);
        }

        // TODO: include current dir? this could be an option.
        let rel_path = util::class_path_slice_to_relative_path(class_path);
        self.load_from_rel_path(class_directories, class_file_id, rel_path)?;
        Ok(class_file_id)
    }

    /// Note: the id should already be registered
    pub fn load_by_class_path_id(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_file_id: ClassId,
    ) -> Result<(), LoadClassFileError> {
        if self.contains_key(&class_file_id) {
            return Ok(());
        }

        let _span_ = span!(Level::TRACE, "CF::load_by_class_path_id",).entered();
        info!("=> CF{:?}", class_names.tpath(class_file_id));

        let (class_name, class_info) = class_names
            .name_from_gcid(class_file_id)
            .map_err(LoadClassFileError::BadId)?;
        debug_assert!(!class_name.is_empty());

        if !class_info.has_class_file() {
            return Ok(());
        }

        // TODO: Is this the correct way of converting it?
        let path = convert_classfile_text(class_name.0);
        let path = util::access_path_iter(&path);
        let rel_path = util::class_path_iter_to_relative_path(path);
        self.load_from_rel_path(class_directories, class_file_id, rel_path)?;
        Ok(())
    }

    fn load_from_rel_path(
        &mut self,
        class_directories: &ClassDirectories,
        id: ClassId,
        rel_path: PathBuf,
    ) -> Result<(), LoadClassFileError> {
        if self.contains_key(&id) {
            // It has already been loaded
            return Ok(());
        }

        let class_file = direct_load_class_file_from_rel_path(class_directories, id, rel_path)?;
        self.set_at(id, class_file);

        Ok(())
    }
}

/// Tries converting cesu8-java-style strings into Rust's utf8 strings
/// This tries to avoid allocating but may not be able to avoid it
#[must_use]
pub fn convert_classfile_text(bytes: &[u8]) -> Cow<str> {
    cesu8::from_java_cesu8(bytes).unwrap_or_else(|_| String::from_utf8_lossy(bytes))
}

/// The id must be defined inside of the given class names
pub fn direct_load_class_file_by_id(
    class_directories: &ClassDirectories,
    class_names: &ClassNames,
    class_file_id: ClassId,
) -> Result<Option<ClassFileData>, LoadClassFileError> {
    let (class_name, class_info) = class_names
        .name_from_gcid(class_file_id)
        .map_err(LoadClassFileError::BadId)?;
    debug_assert!(!class_name.is_empty());

    if !class_info.has_class_file() {
        // There's no class file to parse
        return Ok(None);
    }

    let path = convert_classfile_text(class_name.0);
    let path = util::access_path_iter(&path);
    let rel_path = util::class_path_iter_to_relative_path(path);
    direct_load_class_file_from_rel_path(class_directories, class_file_id, rel_path).map(Some)
}

/// Load the class file with the given access path
/// Returns `None` if it would not have a backing class file (ex: Arrays)
pub fn direct_load_class_file_by_class_path_slice<T: AsRef<str>>(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_path: &[T],
) -> Result<Option<ClassFileData>, LoadClassFileError> {
    if let Some(kind) = InternalKind::from_slice(class_path) {
        if !kind.has_class_file() {
            // There is no class file to parse
            return Ok(None);
        }
    }

    // TODO: This is probably not accurate for more complex utf8
    let class_file_id =
        class_names.gcid_from_iter_bytes(class_path.iter().map(AsRef::as_ref).map(str::as_bytes));

    let rel_path = util::class_path_slice_to_relative_path(class_path);

    direct_load_class_file_from_rel_path(class_directories, class_file_id, rel_path).map(Some)
}

pub fn direct_load_class_file_by_class_path_iter<'a>(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_path: impl Iterator<Item = &'a str> + Clone,
) -> Result<Option<ClassFileData>, LoadClassFileError> {
    if class_path.clone().next().is_none() {
        return Err(LoadClassFileError::EmptyPath);
    }

    if let Some(kind) = InternalKind::from_iter(class_path.clone()) {
        if !kind.has_class_file() {
            // There is no class file to parse
            return Ok(None);
        }
    }

    let class_file_id = class_names.gcid_from_iter_bytes(class_path.clone().map(str::as_bytes));

    let rel_path = util::class_path_iter_to_relative_path(class_path);

    direct_load_class_file_from_rel_path(class_directories, class_file_id, rel_path).map(Some)
}

pub fn direct_load_class_file_from_rel_path(
    class_directories: &ClassDirectories,
    id: ClassId,
    rel_path: PathBuf,
) -> Result<ClassFileData, LoadClassFileError> {
    use classfile_parser::parser::ParseData;

    if let Some((file_path, mut file)) = class_directories.load_class_file_with_rel_path(&rel_path)
    {
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(LoadClassFileError::ReadError)?;
        let data = Rc::from(data);

        // TODO: Better errors
        let (rem_data, class_file) = class_parser_opt(ParseData::new(&data))
            .map_err(|x| format!("{:?}", x))
            .map_err(LoadClassFileError::ClassFileParseError)?;
        // TODO: Don't assert
        debug_assert!(rem_data.is_empty());

        Ok(ClassFileData::new(id, file_path, data, class_file))
    } else {
        Err(LoadClassFileError::NonexistentFile(rel_path))
    }
}

// TODO: Will this behave incorrectly for classes which extend arrays? Those are incorrect, but
// should be properly handled.
// TODO: Should we rename these two iteration functions to something else to better represent
// that they include the base class?
/// Provides an 'iterator' over classes as it crawls up from the `class_id` given
/// Note that this *includes* the `class_id` given, and so you may want to skip over it.
#[must_use]
pub fn load_super_classes_iter(class_id: ClassId) -> SuperClassIterator {
    SuperClassIterator {
        scfi: SuperClassFileIterator::new(class_id),
    }
}

/// Provides an 'iterator' over class files as it crawls up from the `class_id` given
/// Note that this *includes* the `class_id` given, and so you may want to skip over it.
#[must_use]
pub fn load_super_class_files_iter(class_file_id: ClassId) -> SuperClassFileIterator {
    SuperClassFileIterator::new(class_file_id)
}

pub fn direct_load_method_from_index(
    class_names: &mut ClassNames,
    class_file: &ClassFileData,
    method_index: MethodIndex,
) -> Result<Method, StepError> {
    let method_id = MethodId::unchecked_compose(class_file.id(), method_index);
    let method = class_file
        .load_method_info_opt_by_index(method_index)
        .map_err(|_| LoadMethodError::NonexistentMethod { id: method_id })?;
    let method = Method::new_from_info(method_id, class_file, class_names, method)?;

    Ok(method)
}

fn method_id_from_desc<'a>(
    class_names: &mut ClassNames,
    class_file: &'a ClassFileData,
    name: &[u8],
    desc: &MethodDescriptor,
) -> Result<(MethodId, MethodInfoOpt), StepError> {
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
            let method_id = MethodId::unchecked_compose(class_file.id(), method_index);

            return Ok((method_id, method_info));
        }
    }

    Err(LoadMethodError::NonexistentMethodName {
        class_id: class_file.id(),
        name: Cow::Owned(name.to_owned()),
    }
    .into())
}

pub fn direct_load_method_from_desc(
    class_names: &mut ClassNames,
    class_file: &ClassFileData,
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
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    method: &Method,
) -> Result<(), StepError> {
    for parameter_type in method.descriptor().parameters().iter().copied() {
        load_descriptor_type(
            classes,
            class_directories,
            class_names,
            class_files,
            packages,
            parameter_type,
        )?;
    }

    if let Some(return_type) = method.descriptor().return_type().copied() {
        load_descriptor_type(
            classes,
            class_directories,
            class_names,
            class_files,
            packages,
            return_type,
        )?;
    }

    Ok(())
}

fn helper_get_overrided_method(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    super_class_file_id: ClassId,
    over_package: Option<PackageId>,
    over_method: &Method,
    over_method_name: Vec<u8>,
) -> Result<Option<MethodId>, StepError> {
    classes.load_class(
        class_directories,
        class_names,
        class_files,
        packages,
        super_class_file_id,
    )?;
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
            class_directories,
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
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    method_id: MethodId,
) -> Result<(), StepError> {
    methods.load_method_from_id(class_directories, class_names, class_files, method_id)?;

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
                class_directories,
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

// TODO: These recursive load super class functions have the potential for cycles
// there should be some way to not have that. Iteration limit is most likely the simplest
// way, and it avoids allocation.
// Theoretically, with cb versions, the user could return an error if they notice
// a cycle, but that is unpleasant and there should at least be simple ways to do it.

pub fn verify_code_exceptions(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    method: &mut Method,
) -> Result<(), StepError> {
    fn get_class(
        class_files: &ClassFiles,
        method_id: MethodId,
    ) -> Result<&ClassFileData, StepError> {
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
                    class_directories,
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

/// Note: includes itself
pub fn does_extend_class(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &Classes,
    class_id: ClassId,
    desired_super_class_id: ClassId,
) -> Result<bool, StepError> {
    if class_id == desired_super_class_id {
        return Ok(true);
    }

    let super_class_id = if let Some(class) = classes.get(&class_id) {
        class.super_id()
    } else if let Some(class_file) = class_files.get(&class_id) {
        class_file
            .get_super_class_id(class_names)
            .map_err(StepError::ClassFileIndex)?
    } else {
        // The id should have already been registered by now
        class_files.load_by_class_path_id(class_directories, class_names, class_id)?;
        let class_file = class_files
            .get(&class_id)
            .ok_or(StepError::MissingLoadedValue(
                "helper_does_extend_class : class_file",
            ))?;
        class_file
            .get_super_class_id(class_names)
            .map_err(StepError::ClassFileIndex)?
    };

    if let Some(super_class_id) = super_class_id {
        if super_class_id == desired_super_class_id {
            // It does extend it
            Ok(true)
        } else {
            // Crawl further up the tree to see if it extends it
            // Trees should be relatively small so doing recursion probably doesn't matter
            does_extend_class(
                class_directories,
                class_names,
                class_files,
                classes,
                super_class_id,
                desired_super_class_id,
            )
        }
    } else {
        // There was no super class id so we're done here
        Ok(false)
    }
}

// TODO: It would be nice of SuperClassIterator could simply
// be implemented as a normal `.map` over super_class file iterator
// but SCFI borrows fields that this one needs and it wouldn't be able to access
// them.
pub struct SuperClassIterator {
    scfi: SuperClassFileIterator,
}
impl SuperClassIterator {
    pub fn next_item(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        classes: &mut Classes,
        packages: &mut Packages,
    ) -> Option<Result<ClassId, StepError>> {
        match self
            .scfi
            .next_item(class_directories, class_names, class_files)
        {
            Some(Ok(id)) => Some(
                classes
                    .load_class(class_directories, class_names, class_files, packages, id)
                    .map(|_| id),
            ),
            Some(Err(err)) => Some(Err(err)),
            None => None,
        }
    }
}

pub struct SuperClassFileIterator {
    topmost: Option<Result<ClassId, StepError>>,
    had_error: bool,
}
impl SuperClassFileIterator {
    /// Construct the iterator, doing basic processing
    fn new(base_class_id: ClassId) -> SuperClassFileIterator {
        SuperClassFileIterator {
            topmost: Some(Ok(base_class_id)),
            had_error: false,
        }
    }

    // This isn't an iterator, unfortunately, because it needs state paseed into `next_item`
    // to make it usable.
    // We can't simply borrow the fields because the code that is using the iterator likely
    // wants to use them too!
    // TODO: We could maybe make a weird iterator trait that takes in:
    // type Args = (&'a ClassDirectories, &'a mut ClassNames, &'a mut ClassFiles)
    // for its next and then for any iterator methods which we need
    pub fn next_item(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
    ) -> Option<Result<ClassId, StepError>> {
        if self.had_error {
            return None;
        }

        // Get the id, returning the error if there is one
        let topmost = match self.topmost.take() {
            Some(Ok(topmost)) => topmost,
            Some(Err(err)) => {
                self.had_error = true;
                return Some(Err(err));
            }
            // We are now done.
            None => return None,
        };

        // Load the class file by the id
        if let Err(err) = class_files.load_by_class_path_id(class_directories, class_names, topmost)
        {
            self.had_error = true;
            return Some(Err(err.into()));
        }

        // We just loaded it
        let class_file = class_files.get(&topmost).unwrap();

        // Get the super class for next iteration, but we delay checking the error
        self.topmost = class_file
            .get_super_class_id(class_names)
            .map_err(StepError::ClassFileIndex)
            .transpose();

        // The class file was initialized
        Some(Ok(topmost))
    }
}

pub(crate) fn load_basic_descriptor_type(
    classes: &mut Classes,
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    packages: &mut Packages,
    bdesc_type: &DescriptorTypeBasic,
) -> Result<Option<ClassId>, StepError> {
    match bdesc_type {
        DescriptorTypeBasic::Class(class_id) => {
            classes.load_class(
                class_directories,
                class_names,
                class_files,
                packages,
                *class_id,
            )?;
            Ok(Some(*class_id))
        }
        _ => Ok(None),
    }
}

pub(crate) fn load_descriptor_type(
    classes: &mut Classes,
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    packages: &mut Packages,
    desc_type: DescriptorType,
) -> Result<(), StepError> {
    match desc_type {
        DescriptorType::Basic(x) => {
            load_basic_descriptor_type(
                classes,
                class_directories,
                class_names,
                class_files,
                packages,
                &x,
            )?;
            Ok(())
        }
        DescriptorType::Array { level, component } => {
            classes.load_level_array_of_desc_type_basic(
                class_directories,
                class_names,
                class_files,
                packages,
                level,
                component,
            )?;

            Ok(())
        }
    }
}
