#![warn(clippy::pedantic)]
// The way this library is designed has many arguments. Grouping them together would be nice for
// readability, but it makes it harder to minimize dependnecies which has other knock-on effects..
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
// Unfortunately, Clippy isn't smart enough to notice if a function call is trivial and so likely
// does not have an issue in being used in this position.
#![allow(clippy::or_fun_call)]

use std::{collections::HashMap, num::NonZeroUsize, path::Path};

use classfile_parser::{constant_info::ConstantInfo, constant_pool::ConstantPoolIndexRaw};
// use dhat::{Dhat, DhatAlloc};
use rhojvm_base::{
    class::{ArrayClass, ArrayComponentType, ClassAccessFlags, ClassVariant},
    code::stack_map::StackMapError,
    id::{ClassFileId, ClassId, MethodId},
    load_super_classes_iter,
    package::Packages,
    ClassDirectories, ClassFiles, ClassNames, Classes, Methods, StepError,
};
use smallvec::SmallVec;
use stack_map_verifier::{StackMapVerificationLogging, VerifyStackMapGeneralError};
use tracing::info;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;

// #[global_allocator]
// static ALLOCATOR: DhatAlloc = DhatAlloc;

mod formatter;

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
enum Status {
    /// It hasn't been started yet
    NotDone,
    /// It has been started but not yet completed
    Started,
    /// It has been started and completed
    Done,
}
impl Status {
    pub fn into_begun(self) -> Option<BegunStatus> {
        match self {
            Status::NotDone => None,
            Status::Started => Some(BegunStatus::Started),
            Status::Done => Some(BegunStatus::Done),
        }
    }
}
impl Default for Status {
    fn default() -> Self {
        Status::NotDone
    }
}

// TODO: Once we can define type aliases that are of limited allowed variants of another enum, this
// can become simpler.
/// A status that only includes the `Started` and `Done` variants
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum BegunStatus {
    /// It hasn't been started yet
    Started,
    /// It has been started and completed
    Done,
}
impl From<BegunStatus> for Status {
    fn from(val: BegunStatus) -> Self {
        match val {
            BegunStatus::Started => Status::Started,
            BegunStatus::Done => Status::Done,
        }
    }
}

/// Information specific to each class
#[derive(Debug, Default, Clone)]
struct ClassInfo {
    pub created: Status,
    pub verified: Status,
}

pub(crate) struct State {
    entry_point_class: Option<ClassId>,
    conf: StateConfig,

    classes_info: ClassesInfo,
}
impl State {
    fn new(conf: StateConfig) -> Self {
        Self {
            entry_point_class: None,
            conf,

            classes_info: ClassesInfo::default(),
        }
    }

    fn conf(&self) -> &StateConfig {
        &self.conf
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

#[derive(Debug)]
pub enum GeneralError {
    Step(StepError),
    RunInst(RunInstError),
    Verification(VerificationError),
    Resolve(ResolveError),
    /// We expected the class at this id to exist
    /// This likely points to an internal error
    MissingLoadedClass(ClassId),
    /// We expected the class file at this id to exist
    /// This likely points to an internal error
    MissingLoadedClassFile(ClassId),
    BadClassFileIndex(ConstantPoolIndexRaw<ConstantInfo>),
    /// The class version of the file is not supported by this JVM
    UnsupportedClassVersion,
}

impl From<StepError> for GeneralError {
    fn from(err: StepError) -> Self {
        Self::Step(err)
    }
}
impl From<RunInstError> for GeneralError {
    fn from(err: RunInstError) -> Self {
        Self::RunInst(err)
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

#[derive(Debug)]
pub enum RunInstError {
    NoClassFile(ClassFileId),
    NoMethod(MethodId),
    NoCode(MethodId),
    NoInst(MethodId, usize),
    InvalidGetStaticField,
    InvalidFieldRefClass,
    InvalidClassNameIndex,
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
        log_method_name: true,
        log_received_frame: false,
        log_instruction: true,
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
        tracing::error!("failed to initialized class: {:?}", err);
        return;
    }

    // if let Err(err) = initialize_class(
    //     &class_directories,
    //     &mut class_names,
    //     &mut class_files,
    //     &mut classes,
    //     &mut packages,
    //     &mut methods,
    //     &mut state,
    //     entrypoint_id,
    // ) {
    //     tracing::error!("failed to initialized class: {:?}", err);
    //     return;
    // }

    // // Run the main method
    // let string_id = class_names.gcid_from_slice(&["java", "lang", "String"]);
    // let main_name = "main";
    // let main_descriptor = MethodDescriptor::new_void(vec![DescriptorType::single_array(
    //     DescriptorTypeBasic::Class(string_id),
    // )]);
    // let main_method_id = methods
    //     .load_method_from_desc(
    //         &class_directories,
    //         &mut class_names,
    //         &mut class_files,
    //         entrypoint_id,
    //         main_name,
    //         &main_descriptor,
    //     )
    //     .unwrap();

    // state.entry_point_method = Some(main_method_id);
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
    debug_assert_eq!(create_status, BegunStatus::Done);

    // TODO: We are technically supposed to verify the indices of all constant pool entries
    // at this point, rather than throughout the usage of the program like we do currently.
    // TODO; We are also supposed to verify all the field references and method references
    // (in the sense that they are parseably-valid, whether or not they point to a real
    // thing in some class file)

    // Verify Code
    // let class = classes.get(&class_id).ok_or(GeneralError::BadClassFileIndex(class_id))?;
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

    let info = state.classes_info.get_mut_init(class_id);
    info.verified = Status::Done;

    Ok(BegunStatus::Done)
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

    info.created = Status::Started;

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

        for interface_index in interfaces {
            let class_file = class_files
                .get(&class_id)
                .ok_or(GeneralError::MissingLoadedClassFile(class_id))?;

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
    info.created = Status::Done;

    Ok(BegunStatus::Done)
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

// 5.5
// must be verified, prepared, and optionally resolved
// fn pre_initialize_class(
//     class_directories: &ClassDirectories,
//     class_names: &mut ClassNames,
//     class_files: &mut ClassFiles,
//     classes: &mut Classes,
//     packages: &mut Packages,
//     methods: &mut Methods,
//     state: &mut State,
//     class_id: ClassId,
// ) -> Result<(), GeneralError> {
//     // TODO: Technically we don't have to verify according to the type checking rules
//     // for class files < version 50.0
//     // and, if the type checking fails for version == 50.0, then we can choose to
//     // do verification through type inference

//     if state.pre_init_classes.contains(&class_id) {
//         return Ok(());
//     }
//     state.pre_init_classes.push(class_id);

//     // - classIsTypeSafe
//     // Load super classes
//     let mut iter = rhojvm_base::load_super_classes_iter(class_id);

//     // Skip the first class, since that is the base and so it is allowed to be final
//     // We store the latest class so that we can update it and use it for errors
//     // and checking if the topmost class is Object
//     let mut latest_class = iter
//         .next_item(
//             class_directories,
//             class_names,
//             class_files,
//             classes,
//             packages,
//         )
//         .expect("The base to be included in the processing")?;

//     while let Some(res) = iter.next_item(
//         class_directories,
//         class_names,
//         class_files,
//         classes,
//         packages,
//     ) {
//         let super_class_id = res?;

//         // TODO: Are we intended to preinitialize the entire super-chain?
//         let class = classes.get(&super_class_id).unwrap();
//         let access_flags = class.access_flags();
//         if access_flags.contains(ClassAccessFlags::FINAL) {
//             return Err(VerificationError::SuperClassWasFinal {
//                 base_class_id: latest_class,
//                 super_class_id,
//             }
//             .into());
//         }

//         // We only set this after the check so that we can return the base class
//         latest_class = super_class_id;
//     }

//     // verify that topmost class is object
//     if latest_class != state.object_id {
//         return Err(VerificationError::MostSuperClassNonObject {
//             base_class_id: class_id,
//             most_super_class_id: latest_class,
//         }
//         .into());
//     }

//     verify_type_safe_methods(
//         class_directories,
//         class_names,
//         class_files,
//         classes,
//         packages,
//         methods,
//         state,
//         class_id,
//     )?;

//     Ok(())
// }

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

// // 5.5
// fn initialize_class(
//     class_directories: &ClassDirectories,
//     class_names: &mut ClassNames,
//     class_files: &mut ClassFiles,
//     classes: &mut Classes,
//     packages: &mut Packages,
//     methods: &mut Methods,
//     state: &mut State,
//     class_id: ClassId,
// ) -> Result<(), GeneralError> {
//     if state.init_classes.contains(&class_id) {
//         return Ok(());
//     }
//     state.init_classes.push(class_id);

//     pre_initialize_class(
//         class_directories,
//         class_names,
//         class_files,
//         classes,
//         packages,
//         methods,
//         state,
//         class_id,
//     )?;

//     let class = classes.get(&class_id).unwrap().as_class().unwrap();
//     // TODO: It would be nice if we could somehow avoid collecting to a vec
//     let method_ids = class.iter_method_ids().collect::<Vec<_>>();
//     for method_id in method_ids {
//         methods.load_method_from_id(class_directories, class_names, class_files, method_id)?;
//         let method = methods.get(&method_id).unwrap();
//         rhojvm_base::load_method_descriptor_types(
//             class_directories,
//             class_names,
//             class_files,
//             classes,
//             packages,
//             method,
//         )?;

//         // It would have already been loaded
//         // let method = methods.get(&method_id).unwrap();
//         // let parameters = method.descriptor().parameters().to_owned();
//         // let return_type = method.descriptor().return_type().map(Clone::clone);
//         // for parameter in parameters {
//         //     if let DescriptorType::Basic(DescriptorTypeBasic::Class(id)) = parameter {
//         //         initialize_class(
//         //             class_directories,
//         //             class_names,
//         //             class_files,
//         //             classes,
//         //             packages,
//         //             methods,
//         //             state,
//         //             id,
//         //         )?;
//         //     }
//         // }

//         // if let Some(DescriptorType::Basic(DescriptorTypeBasic::Class(id))) = return_type {
//         //     initialize_class(
//         //         class_directories,
//         //         class_names,
//         //         class_files,
//         //         classes,
//         //         packages,
//         //         methods,
//         //         state,
//         //         id,
//         //     )?;
//         // }
//     }

//     let class = classes.get(&class_id).unwrap().as_class().unwrap();
//     if let Some(super_class) = class.super_id() {
//         initialize_class(
//             class_directories,
//             class_names,
//             class_files,
//             classes,
//             packages,
//             methods,
//             state,
//             super_class,
//         )?;
//     }

//     Ok(())
// }

// fn run_method_code(
//     class_directories: &ClassDirectories,
//     class_names: &mut ClassNames,
//     class_files: &mut ClassFiles,
//     classes: &mut Classes,
//     packages: &mut Packages,
//     methods: &mut Methods,
//     state: &mut State,
//     method_id: MethodId,
// ) -> Result<(), GeneralError> {
//     let method = methods.get_mut(&method_id).unwrap();
//     method.load_code(class_files)?;

//     let inst_count = {
//         let method = methods.get(&method_id).unwrap();
//         let code = method.code().unwrap();
//         code.instructions().len()
//     };
//     for index in 0..inst_count {
//         run_inst(
//             class_directories,
//             class_names,
//             class_files,
//             classes,
//             packages,
//             methods,
//             state,
//             method_id,
//             index,
//         )?;
//     }

//     Ok(())
// }

// fn run_inst(
//     class_directories: &ClassDirectories,
//     class_names: &mut ClassNames,
//     class_files: &mut ClassFiles,
//     classes: &mut Classes,
//     packages: &mut Packages,
//     methods: &mut Methods,
//     state: &mut State,
//     method_id: MethodId,
//     inst_index: usize,
// ) -> Result<(), RunInstError> {
//     use rhojvm_base::code::op::GetStatic;
//     let (class_id, _) = method_id.decompose();

//     let class_file = class_files
//         .get(&class_id)
//         .ok_or(RunInstError::NoClassFile(class_id))?;
//     let method = methods
//         .get(&method_id)
//         .ok_or(RunInstError::NoMethod(method_id))?;
//     let code = method.code().ok_or(RunInstError::NoCode(method_id))?;

//     let (_, inst) = code
//         .instructions()
//         .get(inst_index)
//         .ok_or(RunInstError::NoInst(method_id, inst_index))?
//         .clone();
//     match inst {
//         InstM::IntAdd(_) => {}
//         InstM::GetStatic(GetStatic { index }) => {
//             let field = class_file
//                 .get_t(index)
//                 .ok_or(RunInstError::InvalidGetStaticField)?;
//             let class = class_file
//                 .get_t(field.class_index)
//                 .ok_or(RunInstError::InvalidFieldRefClass)?;
//             let class_name = class_file
//                 .get_text_t(class.name_index)
//                 .ok_or(RunInstError::InvalidClassNameIndex)?;
//         }
//         _ => panic!("Unhandled Instruction at {}: {:#?}", inst_index, inst),
//     }

//     Ok(())
// }
