#![warn(clippy::pedantic)]
// The way this library is designed has many arguments. Grouping them together would be nice for
// readability, but it makes it harder to minimize dependnecies which has other knock-on effects..
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use std::{borrow::Cow, num::NonZeroUsize, path::Path};

use rhojvm_base::{
    class::{ClassAccessFlags, ClassVariant},
    code::{
        method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
        op::{InstM, IntAdd},
        stack_map::StackMapError,
    },
    id::{ClassFileId, ClassId, MethodId},
    package::Packages,
    ClassDirectories, ClassFiles, ClassNames, Classes, Methods, StepError,
};
use stack_map_verifier::{StackMapVerificationLogging, VerifyStackMapGeneralError};
use tracing::info;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;

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

pub(crate) struct State {
    object_id: ClassId,
    entry_point_class: Option<ClassId>,
    entry_point_method: Option<MethodId>,
    conf: StateConfig,

    pre_init_classes: Vec<ClassId>,
    init_classes: Vec<ClassId>,
}
impl State {
    fn new(class_names: &mut ClassNames, conf: StateConfig) -> Self {
        let object_id = class_names.object_id();
        Self {
            object_id,
            entry_point_class: None,
            entry_point_method: None,
            conf,

            pre_init_classes: Vec::new(),
            init_classes: Vec::new(),
        }
    }

    fn conf(&self) -> &StateConfig {
        &self.conf
    }
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
    let mut state = State::new(&mut class_names, conf);

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
        tracing::error!("There was an error in initializing a class: {:?}", err);
        return;
    }

    // Run the main method
    let string_id = class_names.gcid_from_slice(&["java", "lang", "String"]);
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
            Cow::Borrowed(main_name),
            &main_descriptor,
        )
        .unwrap();

    state.entry_point_method = Some(main_method_id);

    // #[derive(Default)]
    // struct Stat {
    //     values: Vec<usize>,
    // }
    // impl Stat {
    //     fn push(&mut self, v: impl Into<usize>) {
    //         let v = v.into();
    //         self.values.push(v);
    //     }

    //     fn count(&self) -> usize {
    //         self.values.len()
    //     }

    //     fn sum(&self) -> usize {
    //         self.values.iter().fold(0, |acc, x| acc + x)
    //     }

    //     fn average(&self) -> f64 {
    //         (self.sum() as f64) / (self.count() as f64)
    //     }

    //     fn sort(&mut self) {
    //         self.values.sort();
    //     }

    //     fn percentile(&self, percent: f64) -> usize {
    //         let index = (self.count() as f64 * percent).round() as usize;
    //         return self.values[index];
    //     }
    // }

    // let mut const_pool_size_stat = Stat::default();
    // let mut interface_stat = Stat::default();
    // let mut fields_stat = Stat::default();
    // let mut methods_stat = Stat::default();
    // let mut attributes_stat = Stat::default();

    // let mut field_attributes_stat = Stat::default();
    // let mut method_attributes_stat = Stat::default();

    // for (entry_id, class) in class_files.iter() {
    //     let c = class.get_class_file_unstable();
    //     const_pool_size_stat.push(c.const_pool_size);
    //     interface_stat.push(c.interfaces_count);
    //     fields_stat.push(c.fields_count);
    //     methods_stat.push(c.methods_count);
    //     attributes_stat.push(c.attributes_count);

    //     for field in c.fields.iter() {
    //         field_attributes_stat.push(field.attributes_count);
    //     }

    //     for method in c.methods.iter() {
    //         method_attributes_stat.push(method.attributes_count)
    //     }
    // }
    // const_pool_size_stat.sort();
    // interface_stat.sort();
    // fields_stat.sort();
    // methods_stat.sort();
    // attributes_stat.sort();

    // field_attributes_stat.sort();
    // method_attributes_stat.sort();

    // // Since they are all the same, this is fine.
    // println!("Count: {}", const_pool_size_stat.count());

    // let perc = 0.90;
    // println!(
    //     "Const Pool: {}, Avg: {:?}",
    //     const_pool_size_stat.percentile(perc),
    //     const_pool_size_stat.average()
    // );
    // println!(
    //     "Interface: {}, Avg: {:?}, Sum: {}",
    //     interface_stat.percentile(perc),
    //     interface_stat.average(),
    //     interface_stat.sum(),
    // );
    // println!(
    //     "Fields: {}, Avg: {:?}",
    //     fields_stat.percentile(perc),
    //     fields_stat.average()
    // );
    // println!(
    //     "Methods: {}, Avg: {:?}",
    //     methods_stat.percentile(perc),
    //     methods_stat.average()
    // );
    // println!(
    //     "Attributes: {}, Avg: {:?}",
    //     attributes_stat.percentile(perc),
    //     attributes_stat.average()
    // );

    // println!(
    //     "Field Attributes: {}, Avg: {:?}",
    //     field_attributes_stat.percentile(perc),
    //     field_attributes_stat.average()
    // );

    // println!(
    //     "Method Attributes: {}, Avg: {:?}",
    //     method_attributes_stat.percentile(perc),
    //     method_attributes_stat.average()
    // );
}

#[derive(Debug)]
pub enum GeneralError {
    Step(StepError),
    RunInst(RunInstError),
    Verification(VerificationError),
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
    /// The method should have had Code but it did not
    NoMethodCode {
        method_id: MethodId,
    },
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

// 5.5
// must be verified, prepared, and optionally resolved
fn pre_initialize_class(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    class_id: ClassId,
) -> Result<(), GeneralError> {
    // TODO: Technically we don't have to verify according to the type checking rules
    // for class files < version 50.0
    // and, if the type checking fails for version == 50.0, then we can choose to
    // do verification through type inference

    if state.pre_init_classes.contains(&class_id) {
        return Ok(());
    }
    state.pre_init_classes.push(class_id);

    // - classIsTypeSafe
    // Load super classes
    let mut iter = rhojvm_base::load_super_classes_iter(class_id);

    // Skip the first class, since that is the base and so it is allowed to be final
    // We store the latest class so that we can update it and use it for errors
    // and checking if the topmost class is Object
    let mut latest_class = iter
        .next_item(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
        )
        .expect("The base to be included in the processing")?;

    while let Some(res) = iter.next_item(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
    ) {
        let super_class_id = res?;

        // TODO: Are we intended to preinitialize the entire super-chain?
        let class = classes.get(&super_class_id).unwrap();
        let access_flags = class.access_flags();
        if access_flags.contains(ClassAccessFlags::FINAL) {
            return Err(VerificationError::SuperClassWasFinal {
                base_class_id: latest_class,
                super_class_id,
            }
            .into());
        }

        // We only set this after the check so that we can return the base class
        latest_class = super_class_id;
    }

    // verify that topmost class is object
    if latest_class != state.object_id {
        return Err(VerificationError::MostSuperClassNonObject {
            base_class_id: class_id,
            most_super_class_id: latest_class,
        }
        .into());
    }

    verify_type_safe_methods(
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

fn verify_type_safe_methods(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    class_id: ClassId,
) -> Result<(), GeneralError> {
    classes.load_class(
        class_directories,
        class_names,
        class_files,
        packages,
        class_id,
    )?;

    let class = classes.get(&class_id).unwrap();
    let method_id_iter = match class {
        ClassVariant::Class(class) => class.iter_method_ids(),
        ClassVariant::Array(_) => {
            tracing::warn!("TODO: Skipped verifying ArrayClass methods");
            return Ok(());
        }
    };

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

    let method = methods.get(&method_id).unwrap();
    if method.should_have_code() {
        if method.code().is_none() {
            // We should have code but there was no code!
            return Err(VerificationError::NoMethodCode { method_id }.into());
        }
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
        )?;
    }

    Ok(())
}

// 5.5
fn initialize_class(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    class_id: ClassId,
) -> Result<(), GeneralError> {
    if state.init_classes.contains(&class_id) {
        return Ok(());
    }
    state.init_classes.push(class_id);

    pre_initialize_class(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
        methods,
        state,
        class_id,
    )?;

    let class = classes.get(&class_id).unwrap().as_class().unwrap();
    // TODO: It would be nice if we could somehow avoid collecting to a vec
    let method_ids = class.iter_method_ids().collect::<Vec<_>>();
    for method_id in method_ids {
        methods.load_method_from_id(class_directories, class_names, class_files, method_id)?;
        let method = methods.get(&method_id).unwrap();
        rhojvm_base::load_method_descriptor_types(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            method,
        )?;

        // It would have already been loaded
        let method = methods.get(&method_id).unwrap();
        let parameters = method.descriptor().parameters().to_owned();
        let return_type = method.descriptor().return_type().map(Clone::clone);
        for parameter in parameters {
            if let DescriptorType::Basic(DescriptorTypeBasic::Class(id)) = parameter {
                initialize_class(
                    class_directories,
                    class_names,
                    class_files,
                    classes,
                    packages,
                    methods,
                    state,
                    id,
                )?;
            }
        }

        if let Some(DescriptorType::Basic(DescriptorTypeBasic::Class(id))) = return_type {
            initialize_class(
                class_directories,
                class_names,
                class_files,
                classes,
                packages,
                methods,
                state,
                id,
            )?;
        }
    }

    let class = classes.get(&class_id).unwrap().as_class().unwrap();
    if let Some(super_class) = class.super_id() {
        initialize_class(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            super_class,
        )?;
    }

    Ok(())
}

fn run_method_code(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    method_id: MethodId,
) -> Result<(), GeneralError> {
    let method = methods.get_mut(&method_id).unwrap();
    method.load_code(class_files)?;

    let inst_count = {
        let method = methods.get(&method_id).unwrap();
        let code = method.code().unwrap();
        code.instructions().len()
    };
    for index in 0..inst_count {
        run_inst(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            method_id,
            index,
        )?;
    }

    Ok(())
}

fn run_inst(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    method_id: MethodId,
    inst_index: usize,
) -> Result<(), RunInstError> {
    use rhojvm_base::code::op::GetStatic;
    let (class_id, _) = method_id.decompose();

    let class_file = class_files
        .get(&class_id)
        .ok_or(RunInstError::NoClassFile(class_id))?;
    let method = methods
        .get(&method_id)
        .ok_or(RunInstError::NoMethod(method_id))?;
    let code = method.code().ok_or(RunInstError::NoCode(method_id))?;

    let (_, inst) = code
        .instructions()
        .get(inst_index)
        .ok_or(RunInstError::NoInst(method_id, inst_index))?
        .clone();
    match inst {
        InstM::IntAdd(_) => {}
        InstM::GetStatic(GetStatic { index }) => {
            let field = class_file
                .get_t(index)
                .ok_or(RunInstError::InvalidGetStaticField)?;
            let class = class_file
                .get_t(field.class_index)
                .ok_or(RunInstError::InvalidFieldRefClass)?;
            let class_name = class_file
                .get_text_t(class.name_index)
                .ok_or(RunInstError::InvalidClassNameIndex)?;
        }
        _ => panic!("Unhandled Instruction at {}: {:#?}", inst_index, inst),
    }

    Ok(())
}
