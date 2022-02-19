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

use std::{borrow::Cow, collections::HashMap, num::NonZeroUsize, path::Path};

use class_instance::{Field, FieldAccess, Fields, Instance, StaticClassInstance};
use classfile_parser::{
    constant_info::{ClassConstant, ConstantInfo},
    constant_pool::ConstantPoolIndexRaw,
    descriptor::{DescriptorType as DescriptorTypeCF, DescriptorTypeError},
    field_info::FieldAccessFlags,
    LoadError,
};
use eval::{EvalError, EvalMethodValue};
use gc::{Gc, GcRef};
// use dhat::{Dhat, DhatAlloc};
use rhojvm_base::{
    class::{ArrayClass, ArrayComponentType, ClassAccessFlags, ClassFileData, ClassVariant},
    code::{
        method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
        stack_map::StackMapError,
        types::{JavaChar, PrimitiveType},
    },
    id::{ClassFileId, ClassId, MethodId},
    load_super_classes_iter,
    package::Packages,
    ClassDirectories, ClassFiles, ClassNames, Classes, LoadMethodError, Methods, StepError,
};
use rv::{RuntimeType, RuntimeValue, RuntimeValuePrimitive};
use smallvec::{smallvec, SmallVec};
use stack_map_verifier::{StackMapVerificationLogging, VerifyStackMapGeneralError};
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;

use crate::{
    class_instance::ReferenceArrayInstance,
    eval::{eval_method, Frame, Locals, ValueException},
};

// #[global_allocator]
// static ALLOCATOR: DhatAlloc = DhatAlloc;

pub mod class_instance;
pub mod eval;
mod formatter;
pub mod gc;
pub mod rv;
pub mod util;

const ENV_TRACING_LEVEL: &str = "RHO_LOG_LEVEL";
const DEFAULT_TRACING_LEVEL: tracing::Level = tracing::Level::WARN;

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

struct StateConfig {
    tracing_level: tracing::Level,
    pub stack_map_verification_logging: StackMapVerificationLogging,
    /// The maximum amount of 4 bytes that a stack can occupy
    /// `None`: No limit on stack size. Though, limits caused by implementation
    /// mean that this may not result in all available memory being used.
    /// It is advised to have some form of limit, though.
    max_stack_size: Option<MaxStackSize>,
}
impl StateConfig {
    fn new() -> StateConfig {
        let tracing_level = StateConfig::compute_tracing_level();
        StateConfig {
            tracing_level,
            stack_map_verification_logging: StackMapVerificationLogging::default(),
            max_stack_size: Some(MaxStackSize::default()),
        }
    }

    fn compute_tracing_level() -> tracing::Level {
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

/// A warning
/// These provide information that isn't a bug but might be indicative of weird
/// decisions in the compiling code, or incorrect reasoning by this JVM implementation
/// As well, there will be settings which allow emitting some info, such as warning
/// if a function has a stack that is of an absurd size.
pub enum Warning {}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Status<T = ()> {
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
enum BegunStatus<T = ()> {
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
struct ClassInfo {
    pub created: Status,
    pub verified: Status,
    pub initialized: Status<ValueException<GcRef<StaticClassInstance>>>,
}

pub struct State {
    entry_point_class: Option<ClassId>,
    conf: StateConfig,

    gc: Gc,

    classes_info: ClassesInfo,

    // Caching of various ids
    char_array_id: Option<ClassId>,

    string_class_id: Option<ClassId>,
    string_char_array_constructor: Option<MethodId>,
}
impl State {
    fn new(conf: StateConfig) -> Self {
        Self {
            entry_point_class: None,
            conf,

            gc: Gc::new(),

            classes_info: ClassesInfo::default(),

            char_array_id: None,
            string_class_id: None,
            string_char_array_constructor: None,
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
            let string_class_id = class_names.gcid_from_array(&["java", "lang", "String"]);
            self.string_class_id = Some(string_class_id);
            string_class_id
        }
    }

    // TODO: Use the direct constructor? I'm unsure if we're guaranteed that it exists, but since
    // it directly takes the char array rather than copying, it would certainly be better.
    /// Get the method id for the String(char[]) constructor
    pub fn get_string_char_array_constructor(
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
            smallvec![DescriptorType::Array {
                level: NonZeroUsize::new(1).unwrap(),
                component: DescriptorTypeBasic::Char,
            }],
            None,
        );

        let id = methods.load_method_from_desc(
            class_directories,
            class_names,
            class_files,
            class_id,
            "<init>",
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
        base_class_id: ClassFileId,
        super_class_id: ClassFileId,
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

struct EmptyWriter;
impl std::io::Write for EmptyWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn make_log_file() -> std::sync::Arc<std::fs::File> {
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("./rho.log")
        .expect("Expected to be able to open log file");
    std::sync::Arc::new(log_file)
}

fn init_logging(conf: &StateConfig) {
    let should_log_console = std::env::var("RHO_LOG_CONSOLE")
        .map(|x| x != "0")
        .unwrap_or(true);
    let should_log_file = std::env::var("RHO_LOG_FILE")
        .map(|x| x != "0")
        .unwrap_or(true);

    let console_layer = if should_log_console {
        Some(
            tracing_subscriber::fmt::Layer::default()
                .with_writer(std::io::stderr)
                .without_time()
                .event_format(formatter::Formatter),
        )
    } else {
        None
    };
    let file_layer = if should_log_file {
        Some(
            tracing_subscriber::fmt::Layer::default()
                .with_writer(make_log_file())
                .without_time()
                .event_format(formatter::Formatter),
        )
    } else {
        None
    };

    let t_subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(conf.tracing_level)
        .without_time()
        .event_format(formatter::Formatter)
        .with_writer(|| EmptyWriter)
        .finish()
        .with(console_layer)
        .with(file_layer);

    // TODO: We may make this jvm a library so this should not be done
    tracing::subscriber::set_global_default(t_subscriber)
        .expect("failed to set global default tracing subscriber");
}

fn main() {
    // let _dhat = Dhat::start_heap_profiling();

    let mut conf = StateConfig::new();
    conf.stack_map_verification_logging = StackMapVerificationLogging {
        log_method_name: false,
        log_received_frame: false,
        log_instruction: false,
        log_stack_modifications: false,
        log_local_variable_modifications: false,
    };

    init_logging(&conf);

    info!("RhoJVM Initializing");

    let mut class_directories: ClassDirectories = ClassDirectories::default();
    let mut class_names: ClassNames = ClassNames::default();
    let mut class_files: ClassFiles = ClassFiles::default();
    let mut classes: Classes = Classes::default();
    let mut packages: Packages = Packages::default();
    let mut methods: Methods = Methods::default();

    let entry_point_cp = ["HelloWorld"];
    let class_dirs = [
        "./rhojvm/ex/lib/rt/",
        "./rhojvm/ex/lib/jce/",
        "./rhojvm/ex/lib/charsets/",
        "./rhojvm/ex/lib/jfr",
        "./rhojvm/ex/lib/jsse",
        "./rhojvm/ex/",
    ];

    for path in class_dirs {
        let path = Path::new(path);
        class_directories
            .add(path)
            .expect("for class directory to properly exist");
    }

    // Initialize State
    let mut state = State::new(conf);

    // Load the entry point
    let entrypoint_id: ClassFileId = class_files
        .load_by_class_path_slice(&class_directories, &mut class_names, &entry_point_cp)
        .unwrap();
    classes
        .load_class(
            &class_directories,
            &mut class_names,
            &mut class_files,
            &mut packages,
            entrypoint_id,
        )
        .unwrap();
    state.entry_point_class = Some(entrypoint_id);

    if let Err(err) = verify_from_entrypoint(
        &class_directories,
        &mut class_names,
        &mut class_files,
        &mut classes,
        &mut packages,
        &mut methods,
        &mut state,
        entrypoint_id,
    ) {
        tracing::error!("failed to verify entrypoint class: {:?}", err);
        return;
    }

    if let Err(err) = initialize_class(
        &class_directories,
        &mut class_names,
        &mut class_files,
        &mut classes,
        &mut packages,
        &mut methods,
        &mut state,
        entrypoint_id,
    ) {
        tracing::error!("failed to initialize entrypoint class {:?}", err);
        return;
    }

    // We get the main method's id so then we can execute it.
    // We could check this early to make so errors from a missing main show up faster, but that is
    // an edge-case, and doesn't matter.
    {
        let string_id = class_names.gcid_from_array(&["java", "lang", "String"]);
        let main_name = "main";
        let main_descriptor = MethodDescriptor::new_void(vec![DescriptorType::single_array(
            DescriptorTypeBasic::Class(string_id),
        )]);
        let main_method_id = methods
            .load_method_from_desc(
                &class_directories,
                &mut class_names,
                &mut class_files,
                entrypoint_id,
                main_name,
                &main_descriptor,
            )
            .expect("Failed to load main method");
        let args = {
            let array_id = class_names
                .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), string_id)
                .expect("Failed to construct type for String[]");
            // TODO: actually construct args
            let array = ReferenceArrayInstance::new(array_id, string_id, Vec::new());
            let array_ref = state.gc.alloc(array);
            array_ref.into_generic()
        };
        let frame = Frame::new_locals(Locals::new_with_array([RuntimeValue::Reference(args)]));
        match eval_method(
            &class_directories,
            &mut class_names,
            &mut class_files,
            &mut classes,
            &mut packages,
            &mut methods,
            &mut state,
            main_method_id,
            frame,
        ) {
            Ok(res) => match res {
                EvalMethodValue::ReturnVoid => (),
                EvalMethodValue::Return(v) => {
                    tracing::warn!("Main returned a value: {:?}", v);
                }
                // TODO: Call the method to get a string from the exception
                EvalMethodValue::Exception(exc) => {
                    tracing::warn!("Main threw an exception! {:?}", exc);
                }
            },
            Err(err) => tracing::error!("There was an error in running the method: {:?}", err),
        }
    }
}

/// Initialize a class
/// This involves creating more information about the class, initializing static fields,
/// verifying it, etc.
/// That does mean that this runs code.
/// # Returns
/// Returns the [`GcRef`] for the static-class instance
pub(crate) fn initialize_class(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    class_id: ClassId,
) -> Result<BegunStatus<ValueException<GcRef<StaticClassInstance>>>, GeneralError> {
    let info = state.classes_info.get_mut_init(class_id);
    if let Some(initialize_begun) = info.initialized.into_begun() {
        return Ok(initialize_begun);
    }

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

    let class = classes.get(&class_id).unwrap();
    if let Some(super_id) = class.super_id() {
        initialize_class(
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

    // TODO: initialize interfaces

    // TODO: Handle arrays

    let class_file = class_files.get(&class_id).unwrap();
    let mut fields = Fields::default();

    // TODO: It'd be nice if we could avoid allocating
    let field_iter = class_file
        .load_field_values_iter()
        .collect::<SmallVec<[_; 8]>>();
    for field_info in field_iter {
        let class_file = class_files.get(&class_id).unwrap();

        let (field_info, constant_index) = field_info.map_err(GeneralError::ClassFileLoad)?;
        if !field_info.access_flags.contains(FieldAccessFlags::STATIC) {
            // We are supposed to ignore the ConstantValue attribute for any fields that are not
            // static
            // As well, we only store the static fields on the StaticClassInstance
            continue;
        }

        let field_name = class_file
            .get_text_t(field_info.name_index)
            .map(Cow::into_owned)
            .ok_or(GeneralError::BadClassFileIndex(
                field_info.name_index.into_generic(),
            ))?;

        let field_descriptor = class_file.get_text_t(field_info.descriptor_index).ok_or(
            GeneralError::BadClassFileIndex(field_info.descriptor_index.into_generic()),
        )?;
        // Parse the type of the field
        let (field_type, rem) = DescriptorTypeCF::parse(field_descriptor.as_ref())
            .map_err(GeneralError::InvalidDescriptorType)?;
        // There shouldn't be any remaining data.
        if !rem.is_empty() {
            return Err(GeneralError::UnparsedFieldType);
        }
        // Convert to alternative descriptor type
        let field_type = DescriptorType::from_class_file_desc(class_names, field_type);
        let field_type: RuntimeType<ClassId> =
            RuntimeType::from_descriptor_type(class_names, field_type).map_err(StepError::BadId)?;
        // Note that we don't initialize field classes

        let is_final = field_info.access_flags.contains(FieldAccessFlags::FINAL);
        let field_access = FieldAccess::from_access_flags(field_info.access_flags);
        if let Some(constant_index) = constant_index {
            let constant = class_file
                .get_t(constant_index)
                .ok_or(GeneralError::BadClassFileIndex(constant_index))?
                .clone();
            let value = match constant {
                ConstantInfo::Integer(x) => RuntimeValuePrimitive::I32(x.value).into(),
                ConstantInfo::Float(x) => RuntimeValuePrimitive::F32(x.value).into(),
                ConstantInfo::Double(x) => RuntimeValuePrimitive::F64(x.value).into(),
                ConstantInfo::Long(x) => RuntimeValuePrimitive::I64(x.value).into(),
                ConstantInfo::String(x) => {
                    // TODO: Better string conversion
                    let text = class_file.get_text_t(x.string_index).ok_or(
                        GeneralError::BadClassFileIndex(x.string_index.into_generic()),
                    )?;
                    let text = text
                        .encode_utf16()
                        .map(|x| RuntimeValuePrimitive::Char(JavaChar(x)))
                        .collect::<Vec<RuntimeValuePrimitive>>();

                    let string_ref = util::construct_string(
                        class_directories,
                        class_names,
                        class_files,
                        classes,
                        packages,
                        methods,
                        state,
                        text,
                    )?;
                    match string_ref {
                        ValueException::Value(string_ref) => {
                            RuntimeValue::Reference(string_ref.into_generic())
                        }
                        // TODO: include information that it was due to initializing a field
                        ValueException::Exception(exc) => {
                            let info = state.classes_info.get_mut_init(class_id);
                            info.initialized = Status::Done(ValueException::Exception(exc));
                            return Ok(BegunStatus::Done(ValueException::Exception(exc)));
                        }
                    }
                }
                // TODO: Better error
                _ => return Err(GeneralError::BadClassFileIndex(constant_index)),
            };

            // TODO: Validate that the value is the right type
            fields.insert(field_name, Field::new(value, is_final, field_access));
        } else {
            // otherwise, we give it the default value for its type
            let default_value = field_type.default_value();
            fields.insert(
                field_name,
                Field::new(default_value, is_final, field_access),
            );
        }
    }

    let instance = StaticClassInstance::new(class_id, fields);
    let instance_ref = state.gc.alloc(instance);

    let info = state.classes_info.get_mut_init(class_id);
    info.initialized = Status::Done(ValueException::Value(instance_ref));

    // TODO: This could potentially be gc'd, we could just store the id?
    // TODO: Should this be done before or after we set initialized?
    let clinit_name = "<clinit>";
    let clinit_desc = MethodDescriptor::new_empty();
    match methods.load_method_from_desc(
        class_directories,
        class_names,
        class_files,
        class_id,
        clinit_name,
        &clinit_desc,
    ) {
        Ok(method_id) => {
            let frame = Frame::default();
            match eval_method(
                class_directories,
                class_names,
                class_files,
                classes,
                packages,
                methods,
                state,
                method_id,
                frame,
            )? {
                EvalMethodValue::ReturnVoid => (),
                EvalMethodValue::Return(_) => tracing::warn!("<clinit> method returned a value"),
                EvalMethodValue::Exception(exc) => {
                    let info = state.classes_info.get_mut_init(class_id);
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

fn verify_from_entrypoint(
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
            let interface_id = class_names.gcid_from_slice(interface_name);
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
                .get_text_t(interface_name)
                .ok_or(GeneralError::BadClassFileIndex(
                    interface_name.into_generic(),
                ))?;
        let interface_id = class_names.gcid_from_cow(interface_name);

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
