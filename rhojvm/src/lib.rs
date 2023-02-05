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
// Not really that useful
#![allow(clippy::redundant_else)]
// Unfortunately, we have to be on nightly to use VaList, thougyh it looks
// like there was some movement recently about getting this stabilized on
// some targets, rather than stabilizing for many targets at once.
// TODO: Provide an option to disable using this feature, which would disable
// the jni functions which rely upon it
#![feature(c_variadic)]

use std::{
    collections::HashMap, num::NonZeroUsize, string::FromUtf16Error, sync::Arc, thread::ThreadId,
};

use class_instance::{
    ClassInstance, FieldId, Instance, StaticClassInstance, StaticFormInstance, ThreadInstance,
};
use classfile_parser::{
    constant_info::{ClassConstant, ConstantInfo},
    constant_pool::ConstantPoolIndexRaw,
    descriptor::DescriptorTypeError,
    field_info::FieldAccessFlags,
    LoadError,
};
use eval::{instances::make_fields, EvalError, EvalMethodValue};
use gc::{Gc, GcRef};
use indexmap::IndexMap;
use jni::native_lib::{FindSymbolError, LoadLibraryError, NativeLibrariesStatic};
use memblock::MemoryBlocks;
use method::MethodInfo;
// use dhat::{Dhat, DhatAlloc};
use rhojvm_base::{
    class::{ArrayClass, ArrayComponentType, ClassAccessFlags, ClassFileInfo, ClassVariant},
    code::{method::MethodDescriptor, stack_map::StackMapError, types::PrimitiveType},
    data::{
        class_file_loader::LoadClassFileError,
        class_files::ClassFiles,
        class_names::ClassNames,
        classes::{load_super_classes_iter, Classes},
        methods::{init_method_overrides, load_method_descriptor_types, LoadMethodError, Methods},
    },
    id::{ClassId, ExactMethodId, MethodId},
    package::Packages,
    StepError,
};
use smallvec::SmallVec;
use stack_map_verifier::{StackMapVerificationLogging, VerifyStackMapGeneralError};
use util::{find_field_with_name, make_exception, Env};

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
pub mod string_intern;
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
    /// Whether it should abort when an `UnsupportedOperationException` is thrown.
    /// This is typically only on for debugging of code, to make it easier to pinpoint in logs.
    pub abort_on_unsupported: bool,
    /// Whether it should log class names after execution of the code
    /// This makes it somewhat easier to diagnose issues where a class is not loaded when it should be
    pub log_class_names: bool,
    /// Whether we should log only control flow instructions, like invokestatic and friends.
    pub log_only_control_flow_insts: bool,
    /// Whether we should skip logging about methods in classes that start with prefixes
    pub log_skip_from_prefixes: bool,
    /// The prefixes to skip over
    pub log_skip_prefixes: Vec<Vec<u8>>,
    /// Custom set properties, like through `-D key=value`
    pub properties: IndexMap<String, String>,
    /// Directories to search for native libraries, on top of the defaults
    pub native_lib_dirs: Vec<String>,
    pub java_home: String,
}
impl StateConfig {
    #[must_use]
    pub fn new() -> StateConfig {
        let tracing_level = StateConfig::compute_tracing_level();
        StateConfig {
            tracing_level,
            stack_map_verification_logging: StackMapVerificationLogging::default(),
            max_stack_size: Some(MaxStackSize::default()),
            abort_on_unsupported: false,
            log_class_names: false,
            log_only_control_flow_insts: false,
            log_skip_from_prefixes: true,
            log_skip_prefixes: Vec::new(),
            properties: IndexMap::new(),
            native_lib_dirs: Vec::new(),
            java_home: String::new(),
        }
    }

    pub fn should_skip_logging(&self, class_names: &ClassNames, class_id: ClassId) -> bool {
        if !self.log_skip_from_prefixes {
            return false;
        }

        self.has_prefix(class_names, class_id)
    }

    fn has_prefix(&self, class_names: &ClassNames, class_id: ClassId) -> bool {
        let Ok((name, _)) = class_names.name_from_gcid(class_id) else {
            return false;
        };

        let name = name.get();

        for prefix in &self.log_skip_prefixes {
            if name.starts_with(prefix) {
                return true;
            }
        }

        false
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
    /// Cached reference to Class<T> for this class
    pub class_ref: Option<GcRef<StaticFormInstance>>,
}

/// State that is per-thread
pub struct ThreadData {
    pub id: ThreadId,
    // This should always be filled after startup
    pub thread_instance: Option<GcRef<ThreadInstance>>,
}
impl ThreadData {
    #[must_use]
    pub fn new(thread_id: ThreadId) -> ThreadData {
        ThreadData {
            id: thread_id,
            thread_instance: None,
        }
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

    /// Holds an exception that occurred in native code
    pub native_exception: Option<GcRef<ClassInstance>>,

    // Caching of various ids
    char_array_id: Option<ClassId>,

    string_class_id: Option<ClassId>,

    /// The field in java/lang/String that holds the `char[]` that is its content.
    string_data_field: Option<FieldId>,

    /// (classId, fieldIndex, flags) in rho/InternalField
    internal_field_field_ids: Option<(FieldId, FieldId, FieldId)>,

    /// internalField in java/lang/reflect/Field
    field_internal_field_id: Option<FieldId>,

    /// name field in java/lang/Package
    package_name_field_id: Option<FieldId>,

    // Cached gcreferences to the Class<T> types for primitives
    pub(crate) void_static_form: Option<GcRef<StaticFormInstance>>,
    pub(crate) byte_static_form: Option<GcRef<StaticFormInstance>>,
    pub(crate) bool_static_form: Option<GcRef<StaticFormInstance>>,
    pub(crate) short_static_form: Option<GcRef<StaticFormInstance>>,
    pub(crate) char_static_form: Option<GcRef<StaticFormInstance>>,
    pub(crate) int_static_form: Option<GcRef<StaticFormInstance>>,
    pub(crate) long_static_form: Option<GcRef<StaticFormInstance>>,
    pub(crate) float_static_form: Option<GcRef<StaticFormInstance>>,
    pub(crate) double_static_form: Option<GcRef<StaticFormInstance>>,
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

            native_exception: None,

            char_array_id: None,
            string_class_id: None,

            string_data_field: None,

            internal_field_field_ids: None,

            field_internal_field_id: None,

            package_name_field_id: None,

            void_static_form: None,
            byte_static_form: None,
            bool_static_form: None,
            short_static_form: None,
            char_static_form: None,
            int_static_form: None,
            long_static_form: None,
            float_static_form: None,
            double_static_form: None,
        }
    }

    fn conf(&self) -> &StateConfig {
        &self.conf
    }

    pub fn fill_native_exception(&mut self, exc: GcRef<ClassInstance>) {
        if self.native_exception.is_some() {
            tracing::warn!(
                "Native exception occurred while there was an unhandled native exception."
            );
            return;
        }

        self.native_exception = Some(exc);
    }

    /// Get the value out of a [`ValueException`], or store the exception
    pub fn extract_value<T>(&mut self, val: ValueException<T>) -> Option<T> {
        match val {
            ValueException::Value(val) => Some(val),
            ValueException::Exception(exc) => {
                self.fill_native_exception(exc);
                None
            }
        }
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

    /// Searches the gc-heap for the Static Class for this specific class
    /// This shouldn't really be used unless needed.
    #[must_use]
    pub fn find_static_class_instance(
        &self,
        class_id: ClassId,
    ) -> Option<GcRef<StaticClassInstance>> {
        for (object_ref, object) in self.gc.iter_ref() {
            let instance = object.value();
            if let Instance::StaticClass(instance) = instance {
                if instance.id == class_id {
                    return Some(object_ref.unchecked_as());
                }
            }
        }

        None
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

    /// The Package class should already be loaded, and the id given should be for it
    /// # Panics
    /// If it can't find the field
    pub(crate) fn get_package_name_field_id(
        &mut self,
        class_files: &ClassFiles,
        class_id: ClassId,
    ) -> Result<FieldId, GeneralError> {
        if let Some(field) = self.package_name_field_id {
            return Ok(field);
        }

        let (field_id, _) = find_field_with_name(class_files, class_id, b"name")?
            .expect("Failed to get field id for internal java/lang/Package#name field");
        self.package_name_field_id = Some(field_id);

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

    pub(crate) fn remove(&mut self, id: ClassId) {
        self.info.remove(&id);
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
impl From<rhojvm_base::class::InvalidConstantPoolIndex> for GeneralError {
    fn from(v: rhojvm_base::class::InvalidConstantPoolIndex) -> Self {
        Self::Eval(EvalError::InvalidConstantPoolIndex(v.0))
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
        method_id: ExactMethodId,
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

    let res = verify_class(env, class_id)?;

    if let ValueException::Exception(exc) = res {
        env.state.classes_info.remove(class_id);
        return Ok(BegunStatus::Done(ValueException::Exception(exc)));
    }

    let class = env.classes.get(&class_id).unwrap();
    if let Some(super_id) = class.super_id() {
        initialize_class(env, super_id)?;
    }

    let (_, cn_info) = env
        .class_names
        .name_from_gcid(class_id)
        .map_err(StepError::BadId)?;
    let is_array = cn_info.is_array();

    // TODO: initialize interfaces

    // TODO: Handle arrays

    let fields = match make_fields(env, class_id, |field_info| {
        field_info.access_flags.contains(FieldAccessFlags::STATIC)
    })? {
        ValueException::Value(fields) => fields,
        ValueException::Exception(exc) => {
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
    if !is_array {
        let clinit_name = b"<clinit>";
        let clinit_desc = MethodDescriptor::new_empty();
        match env.methods.load_method_from_desc(
            &mut env.class_names,
            &mut env.class_files,
            class_id,
            clinit_name,
            &clinit_desc,
        ) {
            Ok(method_id) => {
                let frame = Frame::default();
                match eval_method(env, method_id.into(), frame)? {
                    EvalMethodValue::ReturnVoid => (),
                    EvalMethodValue::Return(_) => {
                        tracing::warn!("<clinit> method returned a value");
                    }
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
    }

    Ok(BegunStatus::Done(ValueException::Value(instance_ref)))
}

pub fn verify_from_entrypoint(env: &mut Env, class_id: ClassId) -> Result<(), GeneralError> {
    verify_class(env, class_id)?;

    Ok(())
}

fn verify_class(
    env: &mut Env,
    class_id: ClassId,
) -> Result<ValueException<BegunStatus>, GeneralError> {
    // Check if it was already verified or it is already in the process of being verified
    // If it is, then we aren't going to do it again.
    let info = env.state.classes_info.get_mut_init(class_id);
    if let Some(verify_begun) = info.verified.into_begun() {
        return Ok(ValueException::Value(verify_begun));
    }

    let create_status = derive_class(env, class_id)?;
    match create_status {
        ValueException::Value(status) => {
            debug_assert_eq!(status, BegunStatus::Done(()));
        }
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    }

    // TODO: We are technically supposed to verify the indices of all constant pool entries
    // at this point, rather than throughout the usage of the program like we do currently.
    // TODO; We are also supposed to verify all the field references and method references
    // (in the sense that they are parseably-valid, whether or not they point to a real
    // thing in some class file)

    let (_, class_info) = env
        .class_names
        .name_from_gcid(class_id)
        .map_err(StepError::BadId)?;
    let has_class_file = class_info.has_class_file();

    // Verify Code
    // TODO: This is technically not completely an accurate check?
    if has_class_file {
        verify_class_methods(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
            &mut env.methods,
            &mut env.state,
            class_id,
        )?;
    }

    // TODO: Do we always have to do this?
    // Verify the super class
    let class = env.classes.get(&class_id).unwrap();
    if let Some(super_id) = class.super_id() {
        verify_class(env, super_id)?;
    }

    let info = env.state.classes_info.get_mut_init(class_id);
    info.verified = Status::Done(());

    Ok(ValueException::Value(BegunStatus::Done(())))
}

/// Assumes `class_id` is already loaded
fn verify_class_methods(
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
    env: &mut Env,
    class_id: ClassId,
) -> Result<ValueException<BegunStatus>, GeneralError> {
    // TODO: I'm uncertain what the checking that L is already an initating loader means for this
    // this might be more something that the caller should handle for that recording linkage errors

    // Check if it was already created or is in the process of being created
    // If it is, then we aren't going to try doing it again, since that
    // could lead down a circular path.
    let info = env.state.classes_info.get_mut_init(class_id);
    if let Some(created_begun) = info.created.into_begun() {
        return Ok(ValueException::Value(created_begun));
    }

    info.created = Status::Started(());

    // First we have to load the class, in case it wasn't already loaded
    let res = env.classes.load_class(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.packages,
        class_id,
    );
    if matches!(
        res,
        Err(StepError::LoadClassFile(
            LoadClassFileError::Nonexistent | LoadClassFileError::NonexistentFile(_)
        ))
    ) {
        let class_not_found_id = env
            .class_names
            .gcid_from_bytes(b"java/lang/ClassNotFoundException");
        let exc = make_exception(
            env,
            class_not_found_id,
            &format!("Failed to find {}", env.class_names.tpath(class_id)),
        )?;
        let exc = exc.flatten();
        return Ok(ValueException::Exception(exc));
    }

    // TODO: Loader stuff?

    // If it has a class file, then we want to check the version
    if let Some(class_file) = env.class_files.get(&class_id) {
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
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
        )
        .expect("Load Super Classes Iter should have at least one entry")?;
    while let Some(super_class_id) = super_iter.next_item(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
    ) {
        let super_class_id = super_class_id?;

        if super_class_id == class_id {
            return Err(VerificationError::CircularInheritance { base_id: class_id }.into());
        }
    }

    let class = env
        .classes
        .get(&class_id)
        .ok_or(GeneralError::MissingLoadedClass(class_id))?;

    if let Some(super_id) = class.super_id() {
        resolve_derive(env, super_id, class_id)?;

        let super_class = env
            .classes
            .get(&super_id)
            .ok_or(GeneralError::MissingLoadedClass(super_id))?;
        if super_class.is_interface() {
            return Err(VerificationError::SuperClassInterface {
                base_id: class_id,
                super_id,
            }
            .into());
        }
    } else if env.class_names.object_id() != class_id {
        // There was no super class and we were not the Object
        // TODO: Should we error on this? THe JVM docs technically don't.
    }

    // An interface must extend Object and nothing else.
    let class = env
        .classes
        .get(&class_id)
        .ok_or(GeneralError::MissingLoadedClass(class_id))?;
    if class.is_interface() && class.super_id() != Some(env.class_names.object_id()) {
        return Err(VerificationError::InterfaceSuperClassNonObject {
            base_id: class_id,
            super_id: class.super_id(),
        }
        .into());
    }

    if env.class_files.contains_key(&class_id) {
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(GeneralError::MissingLoadedClassFile(class_id))?;
        // Collect into a smallvec that should be more than large enough for basically any class
        // since the iterator has a ref to the class file and we need to invalidate it
        let interfaces: SmallVec<[_; 8]> = class_file.interfaces_indices_iter().collect();
        let interfaces: SmallVec<[_; 8]> =
            map_interface_index_small_vec_to_ids(&mut env.class_names, class_file, interfaces)?;

        for interface_id in interfaces {
            // TODO: Check for circular interfaces
            resolve_derive(env, interface_id, class_id)?;
        }
    } else if class.is_array() {
        let array_interfaces = ArrayClass::get_interface_names();
        for interface_name in array_interfaces {
            let interface_id = env.class_names.gcid_from_bytes(interface_name);
            resolve_derive(env, interface_id, class_id)?;
        }
    }

    let info = env.state.classes_info.get_mut_init(class_id);
    info.created = Status::Done(());

    Ok(ValueException::Value(BegunStatus::Done(())))
}

pub(crate) fn map_interface_index_small_vec_to_ids<const N: usize>(
    class_names: &mut ClassNames,
    class_file: &ClassFileInfo,
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
    env: &mut Env,
    class_id: ClassId,
    origin_class_id: ClassId,
) -> Result<(), GeneralError> {
    resolve_class_interface(env, class_id, origin_class_id)?;

    // TODO: This should return the exception!
    derive_class(env, class_id)?;

    Ok(())
}

/// Resolve a class or interface
/// 5.4.3.1
fn resolve_class_interface(
    env: &mut Env,
    class_id: ClassId,
    origin_class_id: ClassId,
) -> Result<(), GeneralError> {
    // TODO: Loader stuff, since the origin loader should be used to load the class id
    env.classes.load_class(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.packages,
        class_id,
    )?;

    let class = env
        .classes
        .get(&class_id)
        .ok_or(GeneralError::MissingLoadedClass(class_id))?;

    if let ClassVariant::Array(array_class) = class {
        if let ArrayComponentType::Class(component_id) = array_class.component_type() {
            // If it has a class as a component, we also have to resolve it
            // TODO: Should the origin be the array's class id or the origin id?
            resolve_derive(env, component_id, class_id)?;
        }
    }

    // TODO: Currently we treat anonymous classes as being able to access anywhere and also being accessible from anywhere, which isn't accurate! I believe it should be using its base class
    // for the accessing
    let is_origin_anon = env
        .class_names
        .name_from_gcid(origin_class_id)
        .map(|(_, info)| info.is_anonymous())
        .map_err(StepError::BadId)?;
    let is_anon = env
        .class_names
        .name_from_gcid(class_id)
        .map(|(_, info)| info.is_anonymous())
        .map_err(StepError::BadId)?;

    if !is_anon
        && !is_origin_anon
        && !can_access_class_from_class(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
            class_id,
            origin_class_id,
        )?
    {
        return Err(ResolveError::InaccessibleClass {
            from: origin_class_id,
            target: class_id,
        }
        .into());
    }

    Ok(())
}

fn can_access_class_from_class(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    target_class_id: ClassId,
    from_class_id: ClassId,
) -> Result<bool, GeneralError> {
    classes.load_class(class_names, class_files, packages, target_class_id)?;
    classes.load_class(class_names, class_files, packages, from_class_id)?;

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
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    method_id: ExactMethodId,
) -> Result<(), GeneralError> {
    let (class_id, method_index) = method_id.decompose();
    // It is generally cheaper to clone since they tend to load it as well..
    let class_file = class_files.get(&class_id).unwrap().clone();
    // let mut methods = Methods::default();
    methods.load_method_from_id(class_names, class_files, method_id)?;
    let method = methods.get(&method_id).unwrap();
    method
        .verify_access_flags()
        .map_err(StepError::VerifyMethod)?;

    load_method_descriptor_types(class_names, class_files, classes, packages, method)?;
    // TODO: Document that this assures that it isn't overriding a final method
    init_method_overrides(
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
