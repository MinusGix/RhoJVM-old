#![warn(clippy::pedantic)]
// The way this library is designed has many arguments. Grouping them together would be nice for
// readability, but it makes it harder to minimize dependnecies which has other knock-on effects..
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
// Unfortunately, Clippy isn't smart enough to notice if a function call is trivial and so likely
// does not have an issue in being used in this position.
#![allow(clippy::or_fun_call)]
#![allow(clippy::module_name_repetitions)]
// TODO: Re-enabling these (or at least panic docs) would be nice, but they make active development
// harder since they highlight the entire function
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
// Too error prone
#![allow(clippy::similar_names)]
// Annoying. Really shouldn't highlight the entire thing.
#![allow(clippy::unnecessary_wraps)]

use std::{
    collections::HashMap, num::NonZeroUsize, string::FromUtf16Error, sync::Arc, thread::ThreadId,
};

use class_instance::{ClassInstance, FieldId, Instance, StaticClassInstance};
use classfile_parser::{
    constant_info::{ClassConstant, ConstantInfo},
    constant_pool::ConstantPoolIndexRaw,
    descriptor::DescriptorTypeError,
    field_info::FieldAccessFlags,
    LoadError,
};
use either::Either;
use eval::{instances::make_fields, EvalError, EvalMethodValue};
use gc::{Gc, GcRef};
use jni::native_lib::{FindSymbolError, LoadLibraryError, NativeLibrariesStatic};
use memblock::MemoryBlocks;
use method::MethodInfo;
// use dhat::{Dhat, DhatAlloc};
use rhojvm_base::{
    class::{ArrayClass, ArrayComponentType, ClassAccessFlags, ClassFileData, ClassVariant},
    code::{
        method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
        stack_map::StackMapError,
        types::PrimitiveType,
    },
    id::{ClassId, MethodId},
    load_super_classes_iter,
    package::Packages,
    ClassDirectories, ClassFiles, ClassNames, Classes, LoadMethodError, Methods, StepError,
};
use smallvec::{smallvec, SmallVec};
use stack_map_verifier::{StackMapVerificationLogging, VerifyStackMapGeneralError};
use util::{find_field_with_name, Env};

use crate::eval::{eval_method, Frame, ValueException};

// #[global_allocator]
// static ALLOCATOR: DhatAlloc = DhatAlloc;

pub mod class_instance;
pub mod eval;
pub mod gc;
pub mod jni;
pub mod memblock;
pub mod method;
pub mod rv;
pub mod util;

pub const ENV_TRACING_LEVEL: &str = "RHO_LOG_LEVEL";
pub const DEFAULT_TRACING_LEVEL: tracing::Level = tracing::Level::WARN;

/// The maximum amount of 4 bytes that a stack can occupy.
/// This stores the amount of 4 bytes that can be used since not having
/// a multiple of four is odd, and can be merely rounded.
#[derive(Debug, Clone)]
pub struct MaxStackSize(NonZeroUsize);
impl MaxStackSize {
    #[must_use]
    /// Construct a max stack size with the number of 4 bytes that a stack can occupy
    /// Note: If receiving bytes, then likely dividing by 4 and rounding down will work well.
    pub fn new(entries: NonZeroUsize) -> MaxStackSize {
        MaxStackSize(entries)
    }

    #[must_use]
    /// The maximum amount of 4 bytes that a stack can occupy
    pub fn count(&self) -> NonZeroUsize {
        self.0
    }

    /// Returns the number of bytes that this means
    /// Returns `None` if the resulting multiplication would overflow.
    pub fn byte_count(&self) -> Option<NonZeroUsize> {
        // TODO: Simplify this once NonZero types have checked_mul
        self.0
            .get()
            .checked_mul(4)
            .map(NonZeroUsize::new)
            // The result can not be zero, so [`NonZeroUsize::new`] cannot fail
            .map(Option::unwrap)
    }
}
impl Default for MaxStackSize {
    fn default() -> Self {
        // TODO: Move this to a constant once you can panic in constants?
        // 1024 KB
        MaxStackSize(NonZeroUsize::new(1024 * 1024).unwrap())
    }
}

pub struct StateConfig {
    pub tracing_level: tracing::Level,
    pub stack_map_verification_logging: StackMapVerificationLogging,
    /// The maximum amount of 4 bytes that a stack can occupy
    /// `None`: No limit on stack size. Though, limits caused by implementation
    /// mean that this may not result in all available memory being used.
    /// It is advised to have some form of limit, though.
    pub max_stack_size: Option<MaxStackSize>,
}
impl StateConfig {
    #[must_use]
    pub fn new() -> StateConfig {
        let tracing_level = StateConfig::compute_tracing_level();
        StateConfig {
            tracing_level,
            stack_map_verification_logging: StackMapVerificationLogging::default(),
            max_stack_size: Some(MaxStackSize::default()),
        }
    }

    #[must_use]
    pub fn compute_tracing_level() -> tracing::Level {
        let env_log = std::env::var(ENV_TRACING_LEVEL);
        if let Ok(env_log) = env_log {
            if env_log.eq_ignore_ascii_case("trace") || env_log == "*" {
                tracing::Level::TRACE
            } else if env_log.eq_ignore_ascii_case("info") {
                tracing::Level::INFO
            } else if env_log.eq_ignore_ascii_case("warn") {
                tracing::Level::WARN
            } else if env_log.eq_ignore_ascii_case("error") {
                tracing::Level::ERROR
            } else {
                DEFAULT_TRACING_LEVEL
            }
        } else {
            DEFAULT_TRACING_LEVEL
        }
    }
}

impl Default for StateConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// A warning
/// These provide information that isn't a bug but might be indicative of weird
/// decisions in the compiling code, or incorrect reasoning by this JVM implementation
/// As well, there will be settings which allow emitting some info, such as warning
/// if a function has a stack that is of an absurd size.
pub enum Warning {}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Status<T = ()> {
    /// It hasn't been started yet
    NotDone,
    /// It has been started but not yet completed
    Started(T),
    /// It has been started and completed
    Done(T),
}
impl<T> Status<T> {
    pub fn into_begun(self) -> Option<BegunStatus<T>> {
        match self {
            Status::NotDone => None,
            Status::Started(v) => Some(BegunStatus::Started(v)),
            Status::Done(v) => Some(BegunStatus::Done(v)),
        }
    }
}
impl<T> Default for Status<T> {
    fn default() -> Self {
        Status::NotDone
    }
}

// TODO: Once we can define type aliases that are of limited allowed variants of another enum, this
// can become simpler.
/// A status that only includes the `Started` and `Done` variants
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BegunStatus<T = ()> {
    /// It hasn't been started yet
    Started(T),
    /// It has been started and completed
    Done(T),
}
impl<T> BegunStatus<T> {
    pub fn into_value(self) -> T {
        match self {
            BegunStatus::Started(v) | BegunStatus::Done(v) => v,
        }
    }
}
impl<T> From<BegunStatus<T>> for Status<T> {
    fn from(val: BegunStatus<T>) -> Self {
        match val {
            BegunStatus::Started(v) => Status::Started(v),
            BegunStatus::Done(v) => Status::Done(v),
        }
    }
}

/// Information specific to each class
#[derive(Debug, Default, Clone)]
pub struct ClassInfo {
    pub created: Status,
    pub verified: Status,
    pub initialized: Status<ValueException<GcRef<StaticClassInstance>>>,
}

/// State that is per-thread
pub struct ThreadData {
    pub id: ThreadId,
}
impl ThreadData {
    #[must_use]
    pub fn new(thread_id: ThreadId) -> ThreadData {
        ThreadData { id: thread_id }
    }
}

pub struct State {
    pub entry_point_class: Option<ClassId>,
    pub conf: StateConfig,

    pub gc: Gc,

    pub mem_blocks: MemoryBlocks,

    pub native: Arc<NativeLibrariesStatic>,

    classes_info: ClassesInfo,
    pub method_info: MethodInfo,

    // Caching of various ids
    char_array_id: Option<ClassId>,

    string_class_id: Option<ClassId>,
    string_char_array_constructor: Option<MethodId>,

    pub(crate) empty_string_ref: Option<GcRef<ClassInstance>>,

    /// The field in java/lang/Class that stores the ClassId
    class_class_id_field: Option<FieldId>,

    /// The field in java/lang/String that holds the `char[]` that is its content.
    string_data_field: Option<FieldId>,

    /// (classId, fieldIndex, flags) in rho/InternalField
    internal_field_field_ids: Option<(FieldId, FieldId, FieldId)>,

    /// internalField in java/lang/reflect/Field
    field_internal_field_id: Option<FieldId>,
}
impl State {
    #[must_use]
    pub fn new(conf: StateConfig) -> Self {
        Self {
            entry_point_class: None,
            conf,

            gc: Gc::new(),

            mem_blocks: MemoryBlocks::default(),

            // The native libraries is wrapped in an `RwLock` since loading libraries is relatively
            // uncommon, but extracting symbols (immutable op) is more common.
            // We leak it so that the libraries will last forever, since unloading seems to be weird
            // and it is hard to ensure that things live long enough and also have lifetimes
            // pointing to the same structure..
            // Leaking it will allow us to also convert symbols into raw function pointers.
            native: Arc::new(NativeLibrariesStatic::new()),

            classes_info: ClassesInfo::default(),
            method_info: MethodInfo::default(),

            char_array_id: None,
            string_class_id: None,
            string_char_array_constructor: None,

            empty_string_ref: None,

            class_class_id_field: None,

            string_data_field: None,

            internal_field_field_ids: None,

            field_internal_field_id: None,
        }
    }

    fn conf(&self) -> &StateConfig {
        &self.conf
    }

    /// Get the id for `char[]`
    pub fn char_array_id(&mut self, class_names: &mut ClassNames) -> ClassId {
        if let Some(id) = self.char_array_id {
            id
        } else {
            let id = class_names.gcid_from_array_of_primitives(PrimitiveType::Char);
            self.char_array_id = Some(id);
            id
        }
    }

    /// Get the id for `java.lang.String`
    fn string_class_id(&mut self, class_names: &mut ClassNames) -> ClassId {
        if let Some(class_id) = self.string_class_id {
            class_id
        } else {
            let string_class_id = class_names.gcid_from_bytes(b"java/lang/String");
            self.string_class_id = Some(string_class_id);
            string_class_id
        }
    }

    // TODO: Should we be using the direct constructor?
    // I'm unsure if we're guaranteed that it exists, but since
    // it directly takes the char array rather than copying, it is better
    /// Get the method id for the String(char[], bool) constructor
    pub(crate) fn get_string_char_array_constructor(
        &mut self,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        methods: &mut Methods,
    ) -> Result<MethodId, StepError> {
        if let Some(constructor) = self.string_char_array_constructor {
            return Ok(constructor);
        }

        let class_id = self.string_class_id(class_names);
        class_files.load_by_class_path_id(class_directories, class_names, class_id)?;

        let char_array_descriptor = MethodDescriptor::new(
            smallvec![
                DescriptorType::Array {
                    level: NonZeroUsize::new(1).unwrap(),
                    component: DescriptorTypeBasic::Char,
                },
                DescriptorType::Basic(DescriptorTypeBasic::Boolean)
            ],
            None,
        );

        let id = methods.load_method_from_desc(
            class_directories,
            class_names,
            class_files,
            class_id,
            b"<init>",
            &char_array_descriptor,
        )?;

        self.string_char_array_constructor = Some(id);

        Ok(id)
    }

    /// Searches the gc-heap for the Static Class for this specific class
    /// This shouldn't really be used unless needed.
    #[must_use]
    pub fn find_static_class_instance(&self, class_id: ClassId) -> Option<GcRef<Instance>> {
        for (object_ref, object) in self.gc.iter_ref() {
            let instance = object.value();
            if let Instance::StaticClass(instance) = instance {
                if instance.id == class_id {
                    return Some(object_ref);
                }
            }
        }

        None
    }

    /// The Class<T> class should already be loaded, and the id given should be for it.
    /// # Panics
    /// If it can't find the field
    pub(crate) fn get_class_class_id_field(
        &mut self,
        class_files: &ClassFiles,
        class_id: ClassId,
    ) -> Result<FieldId, GeneralError> {
        if let Some(field) = self.class_class_id_field {
            return Ok(field);
        }

        let (field_id, _) = find_field_with_name(class_files, class_id, b"classId")?
            .expect("Failed to get field id for internal java/lang/Class#classId field");
        self.class_class_id_field = Some(field_id);

        Ok(field_id)
    }

    /// The String class should already be loaded, and the id given should be for it.
    /// # Panics
    /// If it can't find the field
    pub(crate) fn get_string_data_field(
        &mut self,
        class_files: &ClassFiles,
        class_id: ClassId,
    ) -> Result<FieldId, GeneralError> {
        if let Some(field) = self.string_data_field {
            return Ok(field);
        }

        let (field_id, _) = find_field_with_name(class_files, class_id, b"data")?
            .expect("Failed to get field id for internal java/lang/String#data field");
        self.string_data_field = Some(field_id);

        Ok(field_id)
    }

    /// The Internal Field class should already be loaded, and the id given should be for it
    /// # Panics
    /// If it can't find the field
    pub(crate) fn get_internal_field_ids(
        &mut self,
        class_files: &ClassFiles,
        class_id: ClassId,
    ) -> Result<(FieldId, FieldId, FieldId), GeneralError> {
        if let Some(fields) = self.internal_field_field_ids {
            return Ok(fields);
        }

        let (class_id_field, _) = find_field_with_name(class_files, class_id, b"classId")?
            .expect("Failed to get field id for internal rho/InternalField#classId field");
        let (field_index_field, _) = find_field_with_name(class_files, class_id, b"fieldIndex")?
            .expect("Failed to get field id for internal rho/InternalField#fieldIndex field");
        let (flags_field, _) = find_field_with_name(class_files, class_id, b"flags")?
            .expect("Failed to get field id for internal rho/InternalField#flags field");
        self.internal_field_field_ids = Some((class_id_field, field_index_field, flags_field));

        Ok((class_id_field, field_index_field, flags_field))
    }

    /// The Field class should already be loaded, and the id given should be for it
    /// # Panics
    /// If it can't find the field
    pub(crate) fn get_field_internal_field_id(
        &mut self,
        class_files: &ClassFiles,
        class_id: ClassId,
    ) -> Result<FieldId, GeneralError> {
        if let Some(field) = self.field_internal_field_id {
            return Ok(field);
        }

        let (field_id, _) = find_field_with_name(class_files, class_id, b"internalField")?
            .expect("Failed to get field id for internal java/lang/Field#internalField field");
        self.field_internal_field_id = Some(field_id);

        Ok(field_id)
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ClassesInfo {
    info: HashMap<ClassId, ClassInfo>,
}
impl ClassesInfo {
    pub fn get_mut_init(&mut self, id: ClassId) -> &mut ClassInfo {
        self.info.entry(id).or_default()
    }
}

// TODO: Cleaning up the structure of this error enumeration would be useful
#[derive(Debug)]
pub enum GeneralError {
    Step(StepError),
    Eval(EvalError),
    Verification(VerificationError),
    Resolve(ResolveError),
    ClassFileLoad(LoadError),
    LoadLibrary(LoadLibraryError),
    FindSymbol(FindSymbolError),
    /// We expected the class at this id to exist
    /// This likely points to an internal error
    MissingLoadedClass(ClassId),
    /// We expected the class file at this id to exist
    /// This likely points to an internal error
    MissingLoadedClassFile(ClassId),
    /// We expected the method at this id to exist
    /// This likely points to an internal error
    MissingLoadedMethod(MethodId),
    BadClassFileIndex(ConstantPoolIndexRaw<ConstantInfo>),
    /// The class version of the file is not supported by this JVM
    UnsupportedClassVersion,
    InvalidDescriptorType(DescriptorTypeError),
    UnparsedFieldType,
    /// The string's value store was not named b"value"
    StringNoValueField,
    /// We failed to convert a java string to a rust string
    StringConversionFailure(FromUtf16Error),
}

impl From<StepError> for GeneralError {
    fn from(err: StepError) -> Self {
        Self::Step(err)
    }
}
impl From<EvalError> for GeneralError {
    fn from(err: EvalError) -> Self {
        Self::Eval(err)
    }
}
impl From<VerificationError> for GeneralError {
    fn from(err: VerificationError) -> Self {
        Self::Verification(err)
    }
}
impl From<ResolveError> for GeneralError {
    fn from(err: ResolveError) -> Self {
        Self::Resolve(err)
    }
}
impl From<StackMapError> for GeneralError {
    fn from(err: StackMapError) -> Self {
        Self::Verification(VerificationError::StackMap(err))
    }
}
impl From<VerifyStackMapGeneralError> for GeneralError {
    fn from(err: VerifyStackMapGeneralError) -> Self {
        Self::Verification(VerificationError::VerifyStackMapGeneralError(err))
    }
}
impl From<LoadLibraryError> for GeneralError {
    fn from(err: LoadLibraryError) -> Self {
        Self::LoadLibrary(err)
    }
}
impl From<FindSymbolError> for GeneralError {
    fn from(err: FindSymbolError) -> Self {
        Self::FindSymbol(err)
    }
}

#[derive(Debug)]
pub enum VerificationError {
    StackMap(StackMapError),
    VerifyStackMapGeneralError(VerifyStackMapGeneralError),
    /// Crawling up the chain of a class tree, the topmost class was not `Object`.
    MostSuperClassNonObject {
        /// The class which we were looking at
        base_class_id: ClassId,
        /// The topmost class
        most_super_class_id: ClassId,
    },
    /// The super class of some class was final, which means it should
    /// not have been a super class.
    SuperClassWasFinal {
        /// The immediate base class
        base_class_id: ClassId,
        super_class_id: ClassId,
    },
    /// The super class of a class was an interface, when it should not be
    SuperClassInterface {
        base_id: ClassId,
        super_id: ClassId,
    },
    /// The super class of an interface wasn't `Object`
    InterfaceSuperClassNonObject {
        base_id: ClassId,
        super_id: Option<ClassId>,
    },
    /// The class inherits from itself
    CircularInheritance {
        base_id: ClassId,
    },
    /// The method should have had Code but it did not
    NoMethodCode {
        method_id: MethodId,
    },
}

#[derive(Debug)]
pub enum ResolveError {
    InaccessibleClass { from: ClassId, target: ClassId },
}

/// Initialize a class
/// This involves creating more information about the class, initializing static fields,
/// verifying it, etc.
/// That does mean that this runs code.
/// # Returns
/// Returns the [`GcRef`] for the static-class instance
pub fn initialize_class(
    env: &mut Env<'_>,
    class_id: ClassId,
) -> Result<BegunStatus<ValueException<GcRef<StaticClassInstance>>>, GeneralError> {
    let info = env.state.classes_info.get_mut_init(class_id);
    if let Some(initialize_begun) = info.initialized.into_begun() {
        return Ok(initialize_begun);
    }

    verify_class(
        &env.class_directories,
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
        &mut env.methods,
        &mut env.state,
        class_id,
    )?;

    let class = env.classes.get(&class_id).unwrap();
    if let Some(super_id) = class.super_id() {
        initialize_class(env, super_id)?;
    }

    // TODO: initialize interfaces

    // TODO: Handle arrays

    let fields = match make_fields(env, class_id, |field_info| {
        field_info.access_flags.contains(FieldAccessFlags::STATIC)
    })? {
        Either::Left(fields) => fields,
        Either::Right(exc) => {
            let info = env.state.classes_info.get_mut_init(class_id);
            info.initialized = Status::Done(ValueException::Exception(exc));
            return Ok(BegunStatus::Done(ValueException::Exception(exc)));
        }
    };

    let instance = StaticClassInstance::new(class_id, fields);
    let instance_ref = env.state.gc.alloc(instance);

    let info = env.state.classes_info.get_mut_init(class_id);
    info.initialized = Status::Done(ValueException::Value(instance_ref));

    // TODO: This could potentially be gc'd, we could just store the id?
    // TODO: Should this be done before or after we set initialized?
    let clinit_name = b"<clinit>";
    let clinit_desc = MethodDescriptor::new_empty();
    match env.methods.load_method_from_desc(
        &env.class_directories,
        &mut env.class_names,
        &mut env.class_files,
        class_id,
        clinit_name,
        &clinit_desc,
    ) {
        Ok(method_id) => {
            let frame = Frame::default();
            match eval_method(env, method_id, frame)? {
                EvalMethodValue::ReturnVoid => (),
                EvalMethodValue::Return(_) => tracing::warn!("<clinit> method returned a value"),
                EvalMethodValue::Exception(exc) => {
                    let info = env.state.classes_info.get_mut_init(class_id);
                    info.initialized = Status::Done(ValueException::Exception(exc));
                    return Ok(BegunStatus::Done(ValueException::Exception(exc)));
                }
            }
        }
        // Ignore it, if it doesn't exist
        Err(StepError::LoadMethod(LoadMethodError::NonexistentMethodName { .. })) => (),
        Err(err) => return Err(err.into()),
    }

    Ok(BegunStatus::Done(ValueException::Value(instance_ref)))
}

pub fn verify_from_entrypoint(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    class_id: ClassId,
) -> Result<(), GeneralError> {
    verify_class(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
        methods,
        state,
        class_id,
    )?;

    Ok(())
}

fn verify_class(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    class_id: ClassId,
) -> Result<BegunStatus, GeneralError> {
    // Check if it was already verified or it is already in the process of being verified
    // If it is, then we aren't going to do it again.
    let info = state.classes_info.get_mut_init(class_id);
    if let Some(verify_begun) = info.verified.into_begun() {
        return Ok(verify_begun);
    }

    let create_status = derive_class(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
        methods,
        state,
        class_id,
    )?;
    debug_assert_eq!(create_status, BegunStatus::Done(()));

    // TODO: We are technically supposed to verify the indices of all constant pool entries
    // at this point, rather than throughout the usage of the program like we do currently.
    // TODO; We are also supposed to verify all the field references and method references
    // (in the sense that they are parseably-valid, whether or not they point to a real
    // thing in some class file)

    // Verify Code
    verify_class_methods(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
        methods,
        state,
        class_id,
    )?;

    // TODO: Do we always have to do this?
    // Verify the super class
    let class = classes.get(&class_id).unwrap();
    if let Some(super_id) = class.super_id() {
        verify_class(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            super_id,
        )?;
    }

    let info = state.classes_info.get_mut_init(class_id);
    info.verified = Status::Done(());

    Ok(BegunStatus::Done(()))
}

/// Assumes `class_id` is already loaded
fn verify_class_methods(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    class_id: ClassId,
) -> Result<(), GeneralError> {
    let class = classes
        .get(&class_id)
        .ok_or(GeneralError::MissingLoadedClass(class_id))?;
    let method_id_iter = if let ClassVariant::Class(class) = class {
        let class_file = class_files
            .get(&class_id)
            .ok_or(GeneralError::MissingLoadedClassFile(class_id))?;
        methods
            .load_all_methods_from(class_names, class_file)
            .map_err(StepError::from)?;
        class.iter_method_ids()
    } else {
        // We don't need to verify array methods
        return Ok(());
    };

    // TODO: Verify that if class file version is >=51.0 then jsr and widejsr do not appear
    // TODO: Verify each instruction according to their prolog rules
    // TODO: Verify that we never go over max stack
    // TODO: Are we verifying return types?

    for method_id in method_id_iter {
        verify_type_safe_method(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            method_id,
        )?;
    }

    Ok(())
}

/// Create the class, initializing and doing checks in more detail than [`Classes::load_class`] and
/// similar.
/// Returns the [`BegunStatus`] of the creation, because it may have already been started elsewhere
fn derive_class(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    class_id: ClassId,
) -> Result<BegunStatus, GeneralError> {
    // TODO: I'm uncertain what the checking that L is already an initating loader means for this
    // this might be more something that the caller should handle for that recording linkage errors

    // Check if it was already created or is in the process of being created
    // If it is, then we aren't going to try doing it again, since that
    // could lead down a circular path.
    let info = state.classes_info.get_mut_init(class_id);
    if let Some(created_begun) = info.created.into_begun() {
        return Ok(created_begun);
    }

    info.created = Status::Started(());

    // First we have to load the class, in case it wasn't already loaded
    classes.load_class(
        class_directories,
        class_names,
        class_files,
        packages,
        class_id,
    )?;

    // TODO: Loader stuff?

    // If it has a class file, then we want to check the version
    if let Some(class_file) = class_files.get(&class_id) {
        if let Some(version) = class_file.version() {
            // Currently we don't support pre-jdk8 class files
            // because we have yet to implement the type verifier that they require
            if version.major <= 50 {
                return Err(GeneralError::UnsupportedClassVersion);
            }
        }
    }

    // TODO: Should this be moved after resolving the super class and before creating it?
    // This checks for if the class inherits from itself
    let mut super_iter = load_super_classes_iter(class_id);
    // We skip the base class, since that is the class we just passed in and we already know what it
    // is.
    super_iter
        .next_item(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
        )
        .expect("Load Super Classes Iter should have at least one entry")?;
    while let Some(super_class_id) = super_iter.next_item(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
    ) {
        let super_class_id = super_class_id?;

        if super_class_id == class_id {
            return Err(VerificationError::CircularInheritance { base_id: class_id }.into());
        }
    }

    let class = classes
        .get(&class_id)
        .ok_or(GeneralError::MissingLoadedClass(class_id))?;

    if let Some(super_id) = class.super_id() {
        resolve_derive(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            super_id,
            class_id,
        )?;

        let super_class = classes
            .get(&super_id)
            .ok_or(GeneralError::MissingLoadedClass(super_id))?;
        if super_class.is_interface() {
            return Err(VerificationError::SuperClassInterface {
                base_id: class_id,
                super_id,
            }
            .into());
        }
    } else if class_names.object_id() != class_id {
        // There was no super class and we were not the Object
        // TODO: Should we error on this? THe JVM docs technically don't.
    }

    // An interface must extend Object and nothing else.
    let class = classes
        .get(&class_id)
        .ok_or(GeneralError::MissingLoadedClass(class_id))?;
    if class.is_interface() && class.super_id() != Some(class_names.object_id()) {
        return Err(VerificationError::InterfaceSuperClassNonObject {
            base_id: class_id,
            super_id: class.super_id(),
        }
        .into());
    }

    if class_files.contains_key(&class_id) {
        let class_file = class_files
            .get(&class_id)
            .ok_or(GeneralError::MissingLoadedClassFile(class_id))?;
        // Collect into a smallvec that should be more than large enough for basically any class
        // since the iterator has a ref to the class file and we need to invalidate it
        let interfaces: SmallVec<[_; 8]> = class_file.interfaces_indices_iter().collect();
        let interfaces: SmallVec<[_; 8]> =
            map_interface_index_small_vec_to_ids(class_names, class_file, interfaces)?;

        for interface_id in interfaces {
            // TODO: Check for circular interfaces
            resolve_derive(
                class_directories,
                class_names,
                class_files,
                classes,
                packages,
                methods,
                state,
                interface_id,
                class_id,
            )?;
        }
    } else if class.is_array() {
        let array_interfaces = ArrayClass::get_interface_names();
        for interface_name in array_interfaces {
            let interface_id = class_names.gcid_from_bytes(interface_name);
            resolve_derive(
                class_directories,
                class_names,
                class_files,
                classes,
                packages,
                methods,
                state,
                interface_id,
                class_id,
            )?;
        }
    }

    let info = state.classes_info.get_mut_init(class_id);
    info.created = Status::Done(());

    Ok(BegunStatus::Done(()))
}

pub(crate) fn map_interface_index_small_vec_to_ids<const N: usize>(
    class_names: &mut ClassNames,
    class_file: &ClassFileData,
    interface_indexes: SmallVec<[ConstantPoolIndexRaw<ClassConstant>; N]>,
) -> Result<SmallVec<[ClassId; N]>, GeneralError> {
    let mut interface_ids = SmallVec::new();

    for interface_index in interface_indexes {
        let interface_data =
            class_file
                .get_t(interface_index)
                .ok_or(GeneralError::BadClassFileIndex(
                    interface_index.into_generic(),
                ))?;

        let interface_name = interface_data.name_index;
        let interface_name =
            class_file
                .get_text_b(interface_name)
                .ok_or(GeneralError::BadClassFileIndex(
                    interface_name.into_generic(),
                ))?;
        let interface_id = class_names.gcid_from_bytes(interface_name);

        interface_ids.push(interface_id);
    }

    Ok(interface_ids)
}

/// Resolve a class or interface and create it
/// Equivalent to calling [`resolve_class_interface`] and then [`create_class`]
fn resolve_derive(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    class_id: ClassId,
    origin_class_id: ClassId,
) -> Result<(), GeneralError> {
    resolve_class_interface(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
        methods,
        state,
        class_id,
        origin_class_id,
    )?;

    derive_class(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
        methods,
        state,
        class_id,
    )?;

    Ok(())
}

/// Resolve a class or interface
/// 5.4.3.1
fn resolve_class_interface(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    class_id: ClassId,
    origin_class_id: ClassId,
) -> Result<(), GeneralError> {
    // TODO: Loader stuff, since the origin loader should be used to load the class id
    classes.load_class(
        class_directories,
        class_names,
        class_files,
        packages,
        class_id,
    )?;

    let class = classes
        .get(&class_id)
        .ok_or(GeneralError::MissingLoadedClass(class_id))?;

    if let ClassVariant::Array(array_class) = class {
        if let ArrayComponentType::Class(component_id) = array_class.component_type() {
            // If it has a class as a component, we also have to resolve it
            // TODO: Should the origin be the array's class id or the origin id?
            resolve_derive(
                class_directories,
                class_names,
                class_files,
                classes,
                packages,
                methods,
                state,
                component_id,
                class_id,
            )?;
        }
    }

    if !can_access_class_from_class(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
        class_id,
        origin_class_id,
    )? {
        return Err(ResolveError::InaccessibleClass {
            from: origin_class_id,
            target: class_id,
        }
        .into());
    }

    Ok(())
}

fn can_access_class_from_class(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    target_class_id: ClassId,
    from_class_id: ClassId,
) -> Result<bool, GeneralError> {
    classes.load_class(
        class_directories,
        class_names,
        class_files,
        packages,
        target_class_id,
    )?;
    classes.load_class(
        class_directories,
        class_names,
        class_files,
        packages,
        from_class_id,
    )?;

    let target = classes
        .get(&target_class_id)
        .ok_or(GeneralError::MissingLoadedClass(target_class_id))?;
    if target.access_flags().contains(ClassAccessFlags::PUBLIC) {
        return Ok(true);
    }

    let from = classes
        .get(&from_class_id)
        .ok_or(GeneralError::MissingLoadedClass(from_class_id))?;
    // TODO: This might need to check loaders?

    // TODO: Can subpackages access parent package classes?
    // If they're in the same package, then they can access each other
    if target.package() == from.package() {
        return Ok(true);
    }

    Ok(false)
}

fn verify_type_safe_method(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    method_id: MethodId,
) -> Result<(), GeneralError> {
    let (class_id, method_index) = method_id.decompose();
    // It is generally cheaper to clone since they tend to load it as well..
    let class_file = class_files.get(&class_id).unwrap().clone();
    // let mut methods = Methods::default();
    methods.load_method_from_id(class_directories, class_names, class_files, method_id)?;
    let method = methods.get(&method_id).unwrap();
    method
        .verify_access_flags()
        .map_err(StepError::VerifyMethod)?;

    rhojvm_base::load_method_descriptor_types(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
        method,
    )?;
    // TODO: Document that this assures that it isn't overriding a final method
    rhojvm_base::init_method_overrides(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
        methods,
        method_id,
    )?;

    let method = methods.get_mut(&method_id).unwrap();
    method.load_code(class_files)?;
    let method_code = method.take_code_info();
    if method_code.is_none() && method.should_have_code() {
        // We should have code but there was no code!
        return Err(VerificationError::NoMethodCode { method_id }.into());
    }

    if let Some(method_code) = method_code {
        stack_map_verifier::verify_type_safe_method_stack_map(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state.conf().stack_map_verification_logging.clone(),
            &class_file,
            method_index,
            &method_code,
        )?;

        // Restore the method's code since we have not modified it
        let method = methods.get_mut(&method_id).unwrap();
        method.unchecked_insert_code(method_code);
    }

    Ok(())
}
