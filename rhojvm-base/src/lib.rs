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

use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use class::{
    ArrayClass, ArrayComponentType, Class, ClassFileData, ClassFileIndexError, ClassVariant,
};
use classfile_parser::{
    attribute_info::code_attribute_parser, class_parser, constant_info::Utf8Constant,
    constant_pool::ConstantPoolIndexRaw, method_info::MethodAccessFlags,
};
use code::{
    method::{self, DescriptorType, DescriptorTypeBasic, Method, MethodDescriptor},
    op_ex::InstructionParseError,
    stack_map::StackMapError,
};
use id::{ClassFileId, ClassId, GeneralClassId, MethodId, PackageId};
use package::Packages;
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
    fn register_array_class(&mut self, array_class: ArrayClass) {
        self.set_at(array_class.id(), ClassVariant::Array(array_class));
    }

    pub fn load_class(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        class_file_id: ClassFileId,
    ) -> Result<ClassId, StepError> {
        if self.contains_key(&class_file_id) {
            // It was already loaded
            return Ok(class_file_id);
        }

        let class_name = class_names
            .name_from_gcid(class_file_id)
            .map_err(StepError::BadId)?;
        let _span_ = span!(Level::TRACE, "C::load_class").entered();
        info!("Loading Class {:?}", class_name.path());

        if !class_name.has_class_file() {
            // At the moment, if an array class is loaded, it is already within the classes
            return Ok(class_file_id);
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

        Ok(class_file_id)
    }
}
__make_map!(pub Methods<MethodId, Method>; access);

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
    path: Vec<String>,
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
}
__make_map!(pub ClassNames<GeneralClassId, Name>; access);
impl ClassNames {
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
                        path: vec![class_path.to_string()],
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
        let kind = class_path.clone().next().and_then(InternalKind::from_str);
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
        let kind = class_path.clone().next().and_then(InternalKind::from_str);
        let id = id::hash_access_path_iter(class_path.clone(), true);
        self.map.entry(id).or_insert_with(|| Name {
            internal_kind: kind,
            path: vec![class_path.fold(String::new(), |mut acc, x| {
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

    /// Used for getting nice traces without boilerplate
    pub(crate) fn tpath(&self, id: GeneralClassId) -> &[String] {
        // TODO: Once once_cell or static string alloc is stabilized, we could
        // replace this with a String constant that is more visible like
        // "UNKNOWN_CLASS_NAME"
        const EMPTY_PATH: &[String] = &[String::new()];
        self.path_from_gcid(id).unwrap_or(EMPTY_PATH)
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

        let _span_ = span!(Level::TRACE, "CF::load_from_rel_path").entered();

        info!("Loading CF from {:?}", rel_path);
        if let Some((file_path, mut file)) =
            class_directories.load_class_file_with_rel_path(&rel_path)
        {
            let mut data = Vec::new();
            file.read_to_end(&mut data)
                .map_err(LoadClassFileError::ReadError)?;

            // TODO: Better errors
            let (rem_data, class_file) = class_parser(&data)
                .map_err(|x| format!("{:?}", x))
                .map_err(LoadClassFileError::ClassFileParseError)?;
            debug_assert!(rem_data.is_empty());

            self.set_at(
                id,
                ClassFileData {
                    id,
                    class_file,
                    path: file_path,
                },
            );

            Ok(())
        } else {
            Err(LoadClassFileError::NonexistentFile(rel_path))
        }
    }
}

pub struct ProgramInfo {
    pub class_directories: ClassDirectories,
    conf: Config,
    // == Data ==
    /// Stores a mapping of the general class id to the class access path
    pub class_names: ClassNames,
    pub class_files: ClassFiles,
    pub packages: Packages,
    pub classes: Classes,
    pub methods: Methods,
}
impl ProgramInfo {
    #[must_use]
    pub fn new(conf: Config) -> Self {
        Self {
            class_directories: ClassDirectories::default(),
            conf,
            class_names: ClassNames::new(),
            class_files: ClassFiles::new(),
            packages: Packages::default(),
            classes: Classes::new(),
            methods: Methods::new(),
        }
    }
}

// === Processing ===
impl ProgramInfo {
    // TODO: These recursive load super class functions have the potential for cycles
    // there should be some way to not have that. Iteration limit is most likely the simplest
    // way, and it avoids allocation.
    // Theoretically, with cb versions, the user could return an error if they notice
    // a cycle, but that is unpleasant and there should at least be simple ways to do it.

    /// Note: not really an iterator
    /// Includes the class passed in.
    pub fn load_super_classes_iter(&mut self, class_id: ClassId) -> SuperClassIterator {
        SuperClassIterator {
            scfi: SuperClassFileIterator::new(class_id),
        }
    }

    /// Note: not really an iterator
    /// Includes the class passed in.
    pub fn load_super_class_files_iter(
        &mut self,
        class_file_id: ClassFileId,
    ) -> SuperClassFileIterator {
        SuperClassFileIterator::new(class_file_id)
    }

    pub fn load_method_from_id(&mut self, method_id: MethodId) -> Result<(), StepError> {
        if self.methods.contains_key(&method_id) {
            // It is already loaded
            return Ok(());
        }

        let (class_id, index) = method_id.decompose();
        self.classes.load_class(
            &self.class_directories,
            &mut self.class_names,
            &mut self.class_files,
            &mut self.packages,
            class_id,
        )?;

        // Since we have the class, we should also have the class file
        let class_file = self.class_files.get(&class_id).unwrap();
        let method = class_file
            .get_method(index)
            .ok_or(LoadMethodError::NonexistentMethod { id: method_id })?;

        let method = Method::new_from_info(method_id, class_file, &mut self.class_names, method)?;

        self.methods.set_at(method_id, method);
        Ok(())
    }

    pub fn load_method_from_desc(
        &mut self,
        class_id: ClassId,
        name: Cow<'static, str>,
        desc: &MethodDescriptor,
    ) -> Result<MethodId, StepError> {
        self.classes.load_class(
            &self.class_directories,
            &mut self.class_names,
            &mut self.class_files,
            &mut self.packages,
            class_id,
        )?;

        let class_file = self.class_files.get(&class_id).unwrap();
        let method = class_file
            .methods()
            .iter()
            .enumerate()
            .filter(|(_, x)| class_file.get_text_t(x.name_index) == Some(name.as_ref()))
            .find(|(_, x)| {
                // FIXME: This is awfully inefficient and bad code anyway
                // we could do a streaming-ish version of the method descriptor parser
                // so then no allocations are needed if we aren't keeping it aroundpub t
                let descriptor_text = class_file
                    .get_text_t(x.descriptor_index)
                    .ok_or(LoadMethodError::InvalidDescriptorIndex {
                        index: x.descriptor_index,
                    })
                    .unwrap();
                let x_desc = MethodDescriptor::from_text(descriptor_text, &mut self.class_names)
                    .map_err(LoadMethodError::MethodDescriptorError)
                    .unwrap();
                desc == &x_desc
            });
        if let Some((method_index, method)) = method {
            let method_id = MethodId::unchecked_compose(class_file.id, method_index);

            if self.methods.contains_key(&method_id) {
                // It was already loaded
                return Ok(method_id);
            }

            // TODO: We could move the descriptor since we know it is correct
            let method = Method::new_from_info_with_name(
                method_id,
                class_file,
                &mut self.class_names,
                method,
                name.into_owned(),
            )?;

            self.methods.set_at(method_id, method);
            Ok(method_id)
        } else {
            Err(LoadMethodError::NonexistentMethodName { class_id, name }.into())
        }
    }

    pub fn load_method_descriptor_types(&mut self, method_id: MethodId) -> Result<(), StepError> {
        // Pre-ordering: Method
        self.load_method_from_id(method_id)?;
        let method = self.methods.get(&method_id).unwrap();

        for parameter_type in method.descriptor().parameters().iter().cloned() {
            tracing::info!("\tParam: {:?}", parameter_type);
            load_descriptor_type(
                &mut self.classes,
                &self.class_directories,
                &mut self.class_names,
                &mut self.class_files,
                &mut self.packages,
                parameter_type,
            )?;
        }

        if let Some(return_type) = method.descriptor().return_type().cloned() {
            load_descriptor_type(
                &mut self.classes,
                &self.class_directories,
                &mut self.class_names,
                &mut self.class_files,
                &mut self.packages,
                return_type,
            )?;
        }

        Ok(())
    }

    pub fn init_method_overrides(&mut self, method_id: MethodId) -> Result<(), StepError> {
        self.load_method_from_id(method_id)?;

        let (class_id, _) = method_id.decompose();
        // It should have both the class and method
        let class = self.classes.get(&class_id).unwrap();
        let class = if let ClassVariant::Class(class) = class {
            class
        } else {
            // TODO: There might be one override on them? But if we implement this well,
            // we probably don't need to compute overrides for them anyway.
            eprintln!("Skipped trying to find overrides for an array class");
            return Ok(());
        };
        let package = class.package;

        let method = self.methods.get(&method_id).unwrap();

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
                if let Some(overridden) =
                    self.helper_get_overrided_method(super_class_file_id, package, method_id)?
                {
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

        let method = self
            .methods
            .get_mut(&method_id)
            .ok_or(StepError::MissingLoadedValue(
                "init_method_overrides : method (post)",
            ))?;
        method.overrides = Some(overrides);

        Ok(())
    }

    fn helper_get_overrided_method(
        &mut self,
        super_class_file_id: ClassFileId,
        over_package: Option<PackageId>,
        over_method_id: MethodId,
    ) -> Result<Option<MethodId>, StepError> {
        let _ = self.load_class_variant_from_id(super_class_file_id)?;
        // We reget it, so that it does not believe we have `self` borrowed mutably
        let super_class =
            self.classes
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
            self.class_files
                .get(&super_class_file_id)
                .ok_or(StepError::MissingLoadedValue(
                    "helper_get_overrided_method : super_class_file",
                ))?;
        let over_method =
            self.methods
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
                    let method_desc =
                        MethodDescriptor::from_text(method_desc, &mut self.class_names)
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
            self.helper_get_overrided_method(
                super_super_class_file_id,
                over_package,
                over_method_id,
            )
        } else {
            // There was no method.
            Ok(None)
        }
    }

    pub fn verify_method_access_flags(&mut self, method_id: MethodId) -> Result<(), StepError> {
        self.load_method_from_id(method_id)?;

        let method = self.methods.get(&method_id).unwrap();
        method::verify_method_access_flags(method.access_flags)?;
        Ok(())
    }

    pub fn load_method_code(&mut self, method_id: MethodId) -> Result<(), StepError> {
        self.load_method_from_id(method_id)?;

        let (class_id, _) = method_id.decompose();

        let class_file = self
            .class_files
            .get(&class_id)
            .ok_or(StepError::MissingLoadedValue(
                "load_method_code : class_file",
            ))?;
        let method = self.methods.get(&method_id).unwrap();

        // TODO: Check for code for native/abstract methods to allow malformed
        // versions of them?
        if !method.should_have_code() {
            return Ok(());
        }

        if method.code().is_some() {
            // It already loaded
            return Ok(());
        }

        let code_attr_idx = method
            .attributes()
            .iter()
            .enumerate()
            .find(|(_, x)| {
                class_file
                    .get_text_t(x.attribute_name_index)
                    .map_or(false, |x| x == "Code")
            })
            .map(|(i, _)| i);

        if let Some(attr_idx) = code_attr_idx {
            let code_attr = &method.attributes()[attr_idx];
            let (data_rem, code_attr) = code_attribute_parser(&code_attr.info)
                .map_err(|_| LoadCodeError::InvalidCodeAttribute)?;
            debug_assert!(data_rem.is_empty(), "The remaining data after parsing the code attribute was non-empty. This indicates a bug.");

            // TODO: A config for code parsing that includes information like the class file
            // version?
            // or we could _try_ making it not care and make that a verification step?
            let code = code::parse_code(code_attr).map_err(LoadCodeError::InstructionParse)?;

            self.methods.get_mut(&method_id).unwrap().code = Some(code);
        }

        Ok(())
    }

    pub fn verify_code_exceptions(&mut self, method_id: MethodId) -> Result<(), StepError> {
        fn get_class_method<'cf, 'm>(
            class_files: &'cf ClassFiles,
            methods: &'m Methods,
            method_id: MethodId,
        ) -> Result<(&'cf ClassFileData, &'m Method), StepError> {
            let (class_id, _) = method_id.decompose();
            let class_file = class_files
                .get(&class_id)
                .ok_or(StepError::MissingLoadedValue(
                    "verify_code_exceptions : class_file",
                ))?;
            let method = methods
                .get(&method_id)
                .ok_or(StepError::MissingLoadedValue(
                    "verify_code_exceptions : method",
                ))?;
            Ok((class_file, method))
        }
        fn get_code(method: &Method) -> Result<&CodeInfo, StepError> {
            let code = method.code().ok_or(StepError::MissingLoadedValue(
                "verify_code_exceptions : method.code",
            ))?;
            Ok(code)
        }

        self.load_method_from_id(method_id)?;

        let (_, method) = get_class_method(&self.class_files, &self.methods, method_id)?;
        // TODO: What if it has code despite this
        if !method.should_have_code() {
            return Ok(());
        }

        if method.code.is_none() {
            self.load_method_code(method_id)?;
        }

        let (_, method) = get_class_method(&self.class_files, &self.methods, method_id)?;
        let code = get_code(method)?;

        if code.exception_table().is_empty() {
            // There are no exceptions to verify
            return Ok(());
        }

        let throwable_id = self
            .class_names
            .gcid_from_slice(&["java", "lang", "Throwable"]);

        let exception_table_len = code.exception_table().len();
        for exc_i in 0..exception_table_len {
            {
                let (class_file, method) =
                    get_class_method(&self.class_files, &self.methods, method_id)?;
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
                    let catch_type_id = self.class_names.gcid_from_str(catch_type_name);

                    if !self.does_extend_class(catch_type_id, throwable_id)? {
                        return Err(VerifyCodeExceptionError::NonThrowableCatchType.into());
                    }
                }
            }
            {
                // The above check for the class may have invalidated the references
                let (class_file, method) =
                    get_class_method(&self.class_files, &self.methods, method_id)?;
                let code = get_code(method)?;
                let exc = &code.exception_table()[exc_i];
                debug_assert_eq!(code.exception_table().len(), exception_table_len);

                code.check_exception(class_file, method, exc)?;
            }
        }

        Ok(())
    }
}

// === Helper ===
impl ProgramInfo {
    /// Note: includes itself
    pub fn does_extend_class(
        &mut self,
        class_id: ClassId,
        desired_super_class_id: ClassId,
    ) -> Result<bool, StepError> {
        if class_id == desired_super_class_id {
            return Ok(true);
        }

        let super_class_id = if let Some(class) = self.classes.get(&class_id) {
            class.super_id()
        } else if let Some(class_file) = self.class_files.get(&class_id) {
            class_file
                .get_super_class_id(&mut self.class_names)
                .map_err(StepError::ClassFileIndex)?
        } else {
            // The id should have already been registered by now
            self.class_files.load_by_class_path_id(
                &self.class_directories,
                &mut self.class_names,
                class_id,
            )?;
            let class_file =
                self.class_files
                    .get(&class_id)
                    .ok_or(StepError::MissingLoadedValue(
                        "helper_does_extend_class : class_file",
                    ))?;
            class_file
                .get_super_class_id(&mut self.class_names)
                .map_err(StepError::ClassFileIndex)?
        };

        if let Some(super_class_id) = super_class_id {
            if super_class_id == desired_super_class_id {
                // It does extend it
                Ok(true)
            } else {
                // Crawl further up the tree to see if it extends it
                // Trees should be relatively small so doing recursion probably doesn't matter
                self.does_extend_class(super_class_id, desired_super_class_id)
            }
        } else {
            // There was no super class id so we're done here
            Ok(false)
        }
    }

    pub fn load_class_variant_from_id(
        &mut self,
        class_file_id: ClassFileId,
    ) -> Result<&ClassVariant, StepError> {
        self.classes.load_class(
            &self.class_directories,
            &mut self.class_names,
            &mut self.class_files,
            &mut self.packages,
            class_file_id,
        )?;

        self.get_class_variant(class_file_id)
            .ok_or(StepError::MissingLoadedValue("load_class_variant_from_id"))
    }

    pub fn load_class_file_by_class_path_iter<'a>(
        &mut self,
        class_path: impl Iterator<Item = &'a str> + Clone,
    ) -> Result<ClassFileId, LoadClassFileError> {
        self.class_files.load_by_class_path_iter(
            &self.class_directories,
            &mut self.class_names,
            class_path,
        )
    }

    pub fn load_class_file_by_class_path_slice<T: AsRef<str>>(
        &mut self,
        class_path: &[T],
    ) -> Result<ClassFileId, LoadClassFileError> {
        self.class_files.load_by_class_path_slice(
            &self.class_directories,
            &mut self.class_names,
            class_path,
        )
    }

    /// Note: You may prefer to use `load_class_variant_from_id`, since this method errors
    /// if the class was an array-class.
    pub fn load_class_from_id(&mut self, class_file_id: ClassFileId) -> Result<&Class, StepError> {
        self.load_class_variant_from_id(class_file_id)
            .map(ClassVariant::as_class)
            .transpose()
            .ok_or(StepError::ExpectedNonArrayClass)?
    }
}

// === Getters ===
impl ProgramInfo {
    #[must_use]
    pub fn get_class_variant(&self, class_id: ClassId) -> Option<&ClassVariant> {
        self.classes.get(&class_id)
    }

    #[must_use]
    pub fn get_method(&self, method_id: MethodId) -> Option<&Method> {
        self.methods.get(&method_id)
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
            Some(Ok(id)) => {
                Some(classes.load_class(class_directories, class_names, class_files, packages, id))
            }
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
            let component_id = load_basic_descriptor_type(
                classes,
                class_directories,
                class_names,
                class_files,
                packages,
                &component,
            )?;

            let object_id = class_names.gcid_from_slice(&["java", "lang", "Object"]);

            let access_flags = if let Some(component_id) = component_id {
                class_files.load_by_class_path_id(class_directories, class_names, component_id)?;
                let class_file = class_files.get(&component_id).unwrap();
                class_file.access_flags()
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
            for _ in 0..level.get() {
                // We store it like the descriptor type because that is what it appears as in other
                // places. This does mean that we can't simply use this name as a java-equivalent
                // access path, unfortunately, since an array of ints becomes [I.

                name.insert(0, '[');
                // This has custom handling to keep an array as a lone string
                let id = class_names.gcid_from_str(&name);
                let array = ArrayClass {
                    id,
                    name: name.clone(),
                    super_class: object_id,
                    component_type: prev_type,
                    access_flags,
                };
                classes.register_array_class(array);
                prev_type = ArrayComponentType::Class(id);
            }

            Ok(())
        }
    }
}
