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
    io::Read,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use class::{
    ArrayClass, ArrayComponentType, Class, ClassFileData, ClassFileIndexError, ClassVariant,
};
use classfile_parser::{
    class_parser,
    constant_info::{ClassConstant, Utf8Constant},
    constant_pool::ConstantPoolIndexRaw,
    method_info::{MethodAccessFlags, MethodInfo},
    ClassAccessFlags,
};
use code::{
    method::{self, DescriptorType, DescriptorTypeBasic, Method, MethodDescriptor},
    op_ex::InstructionParseError,
    stack_map::StackMapError,
    types::{PrimitiveType, StackInfoError},
};
use id::{ClassFileId, ClassId, GeneralClassId, MethodId, MethodIndex, PackageId};
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
    pub id: GeneralClassId,
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
        name: Cow<'static, str>,
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
        class_file_id: ClassFileId,
    ) -> Result<(), StepError> {
        if self.contains_key(&class_file_id) {
            // It was already loaded
            return Ok(());
        }

        let class_name = class_names
            .name_from_gcid(class_file_id)
            .map_err(StepError::BadId)?;
        let _span_ = span!(Level::TRACE, "C::load_class").entered();
        info!("Loading Class {:?}", class_name.path());

        if !class_name.has_class_file() {
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
        let super_class_name = class_file
            .get_super_class_name()
            .map_err(LoadClassError::ClassFileIndex)?;
        let super_class_id = super_class_name.map(|x| class_names.gcid_from_str(x));

        let package = {
            let mut package = util::access_path_iter(this_class_name).peekable();
            // TODO: Don't unwrap
            let _class_name = package.next_back().unwrap();
            let package = if package.peek().is_some() {
                let package = packages.iter_parts_create_if_needed(package);
                Some(package)
            } else {
                None
            };

            package
        };

        info!(
            "Got Class Info: {} : {:?}",
            this_class_name, super_class_name
        );

        let class = Class::new(
            class_file_id,
            super_class_id,
            package,
            class_file.access_flags(),
            class_file.methods().len(),
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

        let mut name = component_type
            .to_desc_string(class_names)
            .map_err(StepError::BadId)?;
        name.insert(0, '[');

        let id = class_names.gcid_from_str(&name);
        if let Some(class) = self.get(&id) {
            // It was already loaded
            debug_assert!(matches!(class, ClassVariant::Array(_)));
            return Ok(id);
        }

        let access_flags = {
            // TODO: For normal classes, we only need to load the class file
            self.load_class(
                class_directories,
                class_names,
                class_files,
                packages,
                class_id,
            )?;
            let class = self.get(&class_id).unwrap();
            class.access_flags()
        };
        let array = ArrayClass {
            id,
            name,
            super_class: class_names.object_id(),
            component_type,
            access_flags,
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
        let mut name = component_type
            .to_desc_string(class_names)
            .map_err(StepError::BadId)?;
        name.insert(0, '[');

        let array_id = class_names.gcid_from_str(&name);
        if let Some(class) = self.get(&array_id) {
            // It was already loaded
            debug_assert!(matches!(class, ClassVariant::Array(_)));
            return Ok(array_id);
        }

        let array = ArrayClass::new_unchecked(
            array_id,
            name,
            component_type,
            class_names.object_id(),
            // Since all the types are primitive, we can simply use this
            ClassAccessFlags::PUBLIC,
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
        let component_type = ArrayComponentType::from(prim);
        let mut name = component_type
            .to_desc_string(class_names)
            .map_err(StepError::BadId)?;

        let object_id = class_names.object_id();

        let mut prev_type = component_type;
        let mut last_id = None;
        for _ in 0..level.get() {
            name.insert(0, '[');
            let id = class_names.gcid_from_str(&name);
            if let Some(class) = self.get(&id) {
                // It was already loaded, so do nothing
                debug_assert!(matches!(class, ClassVariant::Array(_)));
            } else {
                let array = ArrayClass {
                    id,
                    name: name.clone(),
                    super_class: object_id,
                    component_type: prev_type,
                    // All primitive types are public
                    access_flags: ClassAccessFlags::PUBLIC,
                };
                self.register_array_class(array);
            }
            prev_type = ArrayComponentType::Class(id);
            last_id = Some(id);
        }

        // Last id must always be filled due to nonzero level
        Ok(last_id.unwrap())
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
        let component_id = load_basic_descriptor_type(
            self,
            class_directories,
            class_names,
            class_files,
            packages,
            &component,
        )?;

        let object_id = class_names.object_id();

        let access_flags = if let Some(component_id) = component_id {
            // TODO: For normal classes, we only need to load the class file
            self.load_class(
                class_directories,
                class_names,
                class_files,
                packages,
                component_id,
            )?;
            let class = self.get(&component_id).unwrap();
            class.access_flags()
        } else {
            // These methods only return none if it was a class, but if it was then it would
            // be in the other branch
            component.access_flags().unwrap()
        };

        let component_type = component.as_array_component_type();

        let mut name = component
            .to_desc_string(class_names)
            .map_err(StepError::BadId)?;
        let mut prev_type = component_type;
        let mut last_id = None;
        for _ in 0..level.get() {
            // We store it like the descriptor type because that is what it appears as in other
            // places. This does mean that we can't simply use this name as a java-equivalent
            // access path, unfortunately, since an array of ints becomes [I.

            name.insert(0, '[');
            // This has custom handling to keep an array as a lone string
            let id = class_names.gcid_from_str(&name);
            if let Some(class) = self.get(&id) {
                // It was already loaded, so do nothing
                debug_assert!(matches!(class, ClassVariant::Array(_)));
            } else {
                let array = ArrayClass {
                    id,
                    name: name.clone(),
                    super_class: object_id,
                    component_type: prev_type,
                    access_flags,
                };
                self.register_array_class(array);
            }
            prev_type = ArrayComponentType::Class(id);
            last_id = Some(id)
        }

        // level being NonZero means that this must be set
        Ok(last_id.unwrap())
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
        let name = class_names
            .name_from_gcid(class_id)
            .map_err(StepError::BadId)?;

        if !name.is_array() {
            // It isn't an array, but that's fine.
            return Ok(None);
        }

        let descriptor: DescriptorTypeCF<'static> = {
            // TODO: Return an error if this doesn't exist, but if it does not then that is sign
            // of an internal bug
            let path = name.path()[0].as_str();
            let (descriptor, remaining) =
                DescriptorTypeCF::parse(path).map_err(StepError::DescriptorTypeError)?;
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
        if let Some(ClassVariant::Array(_class)) = self.get(&class_id) {
            let cloneable = class_names.gcid_from_slice(&["java", "lang", "Cloneable"]);
            if impl_interface_id == cloneable {
                return Ok(true);
            }

            let serializable = class_names.gcid_from_slice(&["java", "io", "Serializable"]);
            if impl_interface_id == serializable {
                return Ok(true);
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
                let interfaces = class_file.interfaces_indices_iter().collect::<Vec<_>>();

                // Check all the topmost indices first
                for interface_index in interfaces.iter().copied() {
                    let interface_constant = class_file
                        .get_t(interface_index)
                        .ok_or(LoadClassError::BadInterfaceIndex(interface_index))?;
                    let interface_name =
                        class_file.get_text_t(interface_constant.name_index).ok_or(
                            LoadClassError::BadInterfaceNameIndex(interface_constant.name_index),
                        )?;
                    let interface_id = class_names.gcid_from_str(interface_name);

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
                let interface_name = class_file.get_text_t(interface_constant.name_index).ok_or(
                    LoadClassError::BadInterfaceNameIndex(interface_constant.name_index),
                )?;
                let interface_id = class_names.gcid_from_str(interface_name);

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
__make_map!(pub Methods<MethodId, Method>; access);
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
        name: Cow<'static, str>,
        desc: &MethodDescriptor,
    ) -> Result<MethodId, StepError> {
        class_files.load_by_class_path_id(class_directories, class_names, class_id)?;
        let class_file = class_files.get(&class_id).unwrap();

        let (method_id, method_info) =
            method_id_from_desc(class_names, class_file, name.as_ref(), desc)?;

        if self.contains_key(&method_id) {
            return Ok(method_id);
        }

        let method = Method::new_from_info_with_name(
            method_id,
            class_file,
            class_names,
            method_info,
            name.into_owned(),
        )?;

        self.set_at(method_id, method);
        Ok(method_id)
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

    fn is_array(&self) -> bool {
        matches!(self, Self::Array)
    }
}
#[derive(Debug, Clone)]
pub struct Name {
    /// Indicates that the class is for an internal type which does not have an actual backing
    /// classfile.
    internal_kind: Option<InternalKind>,
    /// internal arrays only have one entry in this
    path: SmallVec<[String; 6]>,
}
impl Name {
    #[must_use]
    pub fn path(&self) -> &[String] {
        self.path.as_slice()
    }

    #[must_use]
    /// Note: this is about whether it _should_ have a class file
    /// not whether one actually exists
    pub fn has_class_file(&self) -> bool {
        if let Some(kind) = &self.internal_kind {
            kind.has_class_file()
        } else {
            true
        }
    }

    #[must_use]
    pub fn is_array(&self) -> bool {
        matches!(self.internal_kind, Some(InternalKind::Array))
    }
}
__make_map!(pub ClassNames<GeneralClassId, Name>; access);
impl ClassNames {
    /// Gets the id for `java.lang.Object`
    pub fn object_id(&mut self) -> GeneralClassId {
        self.gcid_from_slice(&["java", "lang", "Object"])
    }

    /// Store the class path hash if it doesn't already exist and get the id
    /// If possible, all creations of ids should go through these functions to allow
    /// a mapping from the id back to to the path
    pub fn gcid_from_slice<T: AsRef<str>>(&mut self, class_path: &[T]) -> GeneralClassId {
        let kind = InternalKind::from_slice(class_path);
        if let Some(kind) = &kind {
            if kind.is_array() && class_path.len() > 1 {
                tracing::error!(
                    "gcid_from_slice had internal-kind but had more entries than expected"
                );
            }
        }

        let id = id::hash_access_path_slice(class_path);
        self.map.entry(id).or_insert_with(move || Name {
            internal_kind: kind,
            path: class_path
                .iter()
                .map(AsRef::as_ref)
                .map(ToOwned::to_owned)
                .collect(),
        });
        id
    }

    pub fn gcid_from_str(&mut self, class_path: &str) -> GeneralClassId {
        let kind = InternalKind::from_str(class_path);

        let id = id::hash_access_path(class_path);
        self.map.entry(id).or_insert_with(|| {
            if let Some(kind) = kind {
                match kind {
                    InternalKind::Array => Name {
                        internal_kind: Some(kind),
                        path: smallvec![class_path.to_string()],
                    },
                }
            } else {
                Name {
                    internal_kind: kind,
                    path: util::access_path_iter(class_path)
                        .map(ToOwned::to_owned)
                        .collect(),
                }
            }
        });
        id
    }

    pub fn gcid_from_iter<'a>(
        &mut self,
        class_path: impl Iterator<Item = &'a str> + Clone,
    ) -> GeneralClassId {
        let kind = InternalKind::from_iter(class_path.clone());
        let id = id::hash_access_path_iter(class_path.clone(), false);
        self.map.entry(id).or_insert_with(|| Name {
            internal_kind: kind,
            path: class_path.map(ToOwned::to_owned).collect(),
        });
        id
    }

    /// Turns an iterator of strs into a class id.
    /// This the `single` version, which has the iterator turned into a single string rather than a
    /// a slice of strings.
    pub fn gcid_from_iter_single<'a>(
        &mut self,
        class_path: impl Iterator<Item = &'a str> + Clone,
    ) -> GeneralClassId {
        let kind = InternalKind::from_iter(class_path.clone());
        let id = id::hash_access_path_iter(class_path.clone(), true);
        self.map.entry(id).or_insert_with(|| Name {
            internal_kind: kind,
            path: smallvec![class_path.fold(String::new(), |mut acc, x| {
                acc.push_str(x);
                acc
            })],
        });
        id
    }

    pub fn path_from_gcid(&self, id: GeneralClassId) -> Result<&[String], BadIdError> {
        self.name_from_gcid(id).map(Name::path)
    }

    pub fn name_from_gcid(&self, id: GeneralClassId) -> Result<&Name, BadIdError> {
        self.get(&id).ok_or(BadIdError { id })
    }

    /// A more nicely formatted path from the gcid
    pub fn display_path_from_gcid(&self, id: GeneralClassId) -> Result<String, BadIdError> {
        let path = self.path_from_gcid(id)?;
        let mut result = String::new();
        for (i, part) in path.iter().enumerate() {
            result.push_str(part.as_str());
            if i + 1 < path.len() {
                result.push('.');
            }
        }

        Ok(result)
    }

    /// Used for getting nice traces without boilerplate
    pub(crate) fn tpath(&self, id: GeneralClassId) -> &[String] {
        // TODO: Once once_cell or static string alloc is stabilized, we could
        // replace this with a String constant that is more visible like
        // "UNKNOWN_CLASS_NAME"
        const EMPTY_PATH: &[String] = &[String::new()];
        self.path_from_gcid(id).unwrap_or(EMPTY_PATH)
    }

    pub fn gcid_from_array_of_primitives(&mut self, prim: PrimitiveType) -> ClassId {
        let prefix = prim.as_desc_prefix();
        let iter = ["[", prefix].into_iter();
        let id = id::hash_access_path_iter(iter.clone(), true);

        if !self.contains_key(&id) {
            let name: String = iter.collect();
            let name = Name {
                internal_kind: Some(InternalKind::Array),
                path: smallvec![name],
            };
            self.set_at(id, name);
        }

        id
    }

    pub fn gcid_from_level_array_of_primitives(
        &mut self,
        level: NonZeroUsize,
        prim: PrimitiveType,
    ) -> ClassId {
        let prefix = prim.as_desc_prefix();
        let iter = std::iter::repeat("[").take(level.get());
        let iter = iter.chain([prefix]);
        let id = id::hash_access_path_iter(iter.clone(), true);

        if !self.contains_key(&id) {
            let name: String = iter.collect();
            let name = Name {
                internal_kind: Some(InternalKind::Array),
                path: smallvec![name],
            };
            self.set_at(id, name);
        }

        id
    }

    pub fn gcid_from_level_array_of_class_id(
        &mut self,
        level: NonZeroUsize,
        class_id: ClassId,
    ) -> Result<ClassId, BadIdError> {
        let class_name = self.name_from_gcid(class_id)?;
        let class_path = class_name.path();

        // First we generate an iterator for the id
        // This is so we can check if it already exists without any allocations.
        let first_iter = std::iter::repeat("[").take(level.get());
        // We have to do a branching path here because the type changes..
        let id = if class_name.is_array() {
            // An array only has one entry in the class path
            let component_desc = class_path[0].as_str();
            let iter = first_iter.chain([component_desc]);
            let id = id::hash_access_path_iter(iter.clone(), true);

            // Now, we have to check if it already exists
            if !self.contains_key(&id) {
                let name: String = iter.collect();
                let name = Name {
                    internal_kind: Some(InternalKind::Array),
                    path: smallvec![name],
                };
                self.set_at(id, name);
            }

            id
        } else {
            // To avoid allocations, we have to be a bit rough here
            // Add the opening L for object
            let iter = first_iter.chain(["L"]);
            let class_path_iter = class_path.iter().map(String::as_str);
            let class_path_iter = itertools::intersperse(class_path_iter, "/");
            // Add the object's path
            let iter = iter.chain(class_path_iter);
            // Add the semicolon, indicating the end of the object
            let iter = iter.chain([";"]);
            let id = id::hash_access_path_iter(iter.clone(), true);

            // Now, we have to check if it already exists
            if !self.contains_key(&id) {
                let name: String = iter.collect();
                let name = Name {
                    internal_kind: Some(InternalKind::Array),
                    path: smallvec![name],
                };
                self.set_at(id, name);
            }

            id
        };

        Ok(id)
    }
}

__make_map!(pub ClassFiles<ClassFileId, ClassFileData>; access);
impl ClassFiles {
    pub fn load_by_class_path_iter<'a>(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_path: impl Iterator<Item = &'a str> + Clone,
    ) -> Result<ClassFileId, LoadClassFileError> {
        if class_path.clone().count() == 0 {
            return Err(LoadClassFileError::EmptyPath);
        }

        let class_file_id: ClassFileId = class_names.gcid_from_iter(class_path.clone());
        let class_file_name = class_names.name_from_gcid(class_file_id).unwrap();
        if !class_file_name.has_class_file() || self.contains_key(&class_file_id) {
            return Ok(class_file_id);
        }

        let rel_path = util::class_path_iter_to_relative_path(class_path);
        self.load_from_rel_path(class_directories, class_file_id, rel_path)?;
        Ok(class_file_id)
    }

    pub fn load_by_class_path_slice<T: AsRef<str>>(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_path: &[T],
    ) -> Result<ClassFileId, LoadClassFileError> {
        if class_path.is_empty() {
            return Err(LoadClassFileError::EmptyPath);
        }

        let class_file_id: ClassFileId = class_names.gcid_from_slice(class_path);
        let class_file_name = class_names.name_from_gcid(class_file_id).unwrap();
        debug_assert!(!class_file_name.path().is_empty());
        if !class_file_name.has_class_file() && self.contains_key(&class_file_id) {
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
        class_file_id: ClassFileId,
    ) -> Result<(), LoadClassFileError> {
        if self.contains_key(&class_file_id) {
            return Ok(());
        }

        let _span_ = span!(Level::TRACE, "CF::load_by_class_path_id",).entered();
        info!(
            "Loading CF With CPath {:?}",
            class_names.tpath(class_file_id)
        );

        let class_name = class_names
            .name_from_gcid(class_file_id)
            .map_err(LoadClassFileError::BadId)?;
        debug_assert!(!class_name.path().is_empty());

        if !class_name.has_class_file() {
            return Ok(());
        }

        let rel_path = util::class_path_slice_to_relative_path(class_name.path());
        self.load_from_rel_path(class_directories, class_file_id, rel_path)?;
        Ok(())
    }

    fn load_from_rel_path(
        &mut self,
        class_directories: &ClassDirectories,
        id: ClassFileId,
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

// pub fn direct_load_class_non_array() -> Result<Class, StepError> {

// }

/// The id must be defined inside of the given class names
pub fn direct_load_class_file_by_id(
    class_directories: &ClassDirectories,
    class_names: &ClassNames,
    class_file_id: ClassFileId,
) -> Result<Option<ClassFileData>, LoadClassFileError> {
    let class_name = class_names
        .name_from_gcid(class_file_id)
        .map_err(LoadClassFileError::BadId)?;
    debug_assert!(!class_name.path().is_empty());

    if !class_name.has_class_file() {
        // There's no class file to parse
        return Ok(None);
    }

    let rel_path = util::class_path_slice_to_relative_path(class_name.path());
    direct_load_class_file_from_rel_path(class_directories, class_file_id, rel_path).map(Some)
}

/// Load the class file with the given access path
/// Returns `None` if it would not have a backing class file (ex: Arrays)
pub fn direct_load_class_file_by_class_path_slice<T: AsRef<str>>(
    class_directories: &ClassDirectories,
    class_path: &[T],
) -> Result<Option<ClassFileData>, LoadClassFileError> {
    if let Some(kind) = InternalKind::from_slice(class_path) {
        if !kind.has_class_file() {
            // There is no class file to parse
            return Ok(None);
        }
    }

    let class_file_id = id::hash_access_path_slice(class_path);

    let rel_path = util::class_path_slice_to_relative_path(class_path);

    direct_load_class_file_from_rel_path(class_directories, class_file_id, rel_path).map(Some)
}

pub fn direct_load_class_file_by_class_path_iter<'a>(
    class_directories: &ClassDirectories,
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

    let class_file_id = id::hash_access_path_iter(class_path.clone(), false);

    let rel_path = util::class_path_iter_to_relative_path(class_path);

    direct_load_class_file_from_rel_path(class_directories, class_file_id, rel_path).map(Some)
}

pub fn direct_load_class_file_from_rel_path(
    class_directories: &ClassDirectories,
    id: ClassFileId,
    rel_path: PathBuf,
) -> Result<ClassFileData, LoadClassFileError> {
    if let Some((file_path, mut file)) = class_directories.load_class_file_with_rel_path(&rel_path)
    {
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(LoadClassFileError::ReadError)?;

        // TODO: Better errors
        let (rem_data, class_file) = class_parser(&data)
            .map_err(|x| format!("{:?}", x))
            .map_err(LoadClassFileError::ClassFileParseError)?;
        // TODO: Don't assert
        debug_assert!(rem_data.is_empty());

        Ok(ClassFileData {
            id,
            class_file,
            path: file_path,
        })
    } else {
        Err(LoadClassFileError::NonexistentFile(rel_path))
    }
}

#[must_use]
pub fn load_super_classes_iter(class_id: ClassId) -> SuperClassIterator {
    SuperClassIterator {
        scfi: SuperClassFileIterator::new(class_id),
    }
}

#[must_use]
pub fn load_super_class_files_iter(class_file_id: ClassFileId) -> SuperClassFileIterator {
    SuperClassFileIterator::new(class_file_id)
}

pub fn direct_load_method_from_index(
    class_names: &mut ClassNames,
    class_file: &ClassFileData,
    method_index: MethodIndex,
) -> Result<Method, StepError> {
    let method_id = MethodId::unchecked_compose(class_file.id(), method_index);
    let method = class_file
        .get_method(method_index)
        .ok_or(LoadMethodError::NonexistentMethod { id: method_id })?;
    let method = Method::new_from_info(method_id, class_file, class_names, method)?;

    Ok(method)
}

fn method_id_from_desc<'a>(
    class_names: &mut ClassNames,
    class_file: &'a ClassFileData,
    name: &str,
    desc: &MethodDescriptor,
) -> Result<(MethodId, &'a MethodInfo), StepError> {
    let methods = class_file
        .methods()
        .iter()
        .enumerate()
        .filter(|(_, x)| class_file.get_text_t(x.name_index) == Some(name));

    for (i, method_info) in methods {
        let descriptor_index = method_info.descriptor_index;
        let descriptor_text = class_file.get_text_t(descriptor_index).ok_or(
            LoadMethodError::InvalidDescriptorIndex {
                index: descriptor_index,
            },
        )?;

        if desc
            .is_equal_to_descriptor(class_names, descriptor_text)
            .map_err(LoadMethodError::MethodDescriptorError)?
        {
            let method_id = MethodId::unchecked_compose(class_file.id(), i);

            return Ok((method_id, method_info));
        }
    }

    Err(LoadMethodError::NonexistentMethodName {
        class_id: class_file.id(),
        name: name.to_owned().into(),
    }
    .into())
}

pub fn direct_load_method_from_desc(
    class_names: &mut ClassNames,
    class_file: &ClassFileData,
    name: Cow<'static, str>,
    desc: &MethodDescriptor,
) -> Result<Method, StepError> {
    let (method_id, method_info) =
        method_id_from_desc(class_names, class_file, name.as_ref(), desc)?;

    Ok(Method::new_from_info_with_name(
        method_id,
        class_file,
        class_names,
        method_info,
        name.into_owned(),
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
    methods: &mut Methods,
    super_class_file_id: ClassFileId,
    over_package: Option<PackageId>,
    over_method_id: MethodId,
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
    let over_method = methods
        .get(&over_method_id)
        .ok_or(StepError::MissingLoadedValue(
            "helper_get_overrided_method : over_method",
        ))?;
    for (i, method) in super_class_file.methods().iter().enumerate() {
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

            let method_name = super_class_file.get_text_t(method.name_index).ok_or(
                LoadMethodError::InvalidDescriptorIndex {
                    index: method.name_index,
                },
            )?;
            if method_name == over_method.name {
                // TODO: Don't do allocation for comparison. Either have a way to just directly
                // compare method descriptors with the parsed versions, or a streaming parser
                // for comparison without alloc
                let method_desc = super_class_file.get_text_t(method.descriptor_index).ok_or(
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
            methods,
            super_super_class_file_id,
            over_package,
            over_method_id,
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
        if let Some(super_class_file_id) = class.super_class {
            if let Some(overridden) = helper_get_overrided_method(
                class_directories,
                class_names,
                class_files,
                classes,
                packages,
                methods,
                super_class_file_id,
                package,
                method_id,
            )? {
                vec![MethodOverride::new(overridden)]
            } else {
                Vec::new()
            }
        } else {
            // There is no super class (so, in standard we must be Object), and so we don't have
            // to worry about a method overriding a super-class, since we don't have one and/or
            // we are the penultimate super-class.
            Vec::new()
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

    let throwable_id = class_names.gcid_from_slice(&["java", "lang", "Throwable"]);

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
                    .get_text_t(catch_type.name_index)
                    .ok_or(VerifyCodeExceptionError::InvalidCatchTypeNameIndex)?;
                let catch_type_id = class_names.gcid_from_str(catch_type_name);

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

            code.check_exception(class_file, method, exc)?;
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
    ) -> Option<Result<ClassFileId, StepError>> {
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
    topmost: Option<Result<ClassFileId, StepError>>,
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
    ) -> Option<Result<ClassFileId, StepError>> {
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
