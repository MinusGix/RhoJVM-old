use std::{borrow::Cow, num::NonZeroUsize, path::Path};

use rhojvm_base::{
    class::{ClassAccessFlags, ClassVariant},
    code::{
        method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
        op::InstM,
        stack_map::{StackMapError, VerifyStackMapError},
    },
    id::{ClassFileId, ClassId, MethodId},
    Config, ProgramInfo, StepError,
};
use tracing::info;
use tracing_subscriber::fmt::writer::MakeWriterExt;

mod formatter;
mod stack_map;

const ENV_TRACING_LEVEL: &str = "RHO_LOG_LEVEL";
const DEFAULT_TRACING_LEVEL: tracing::Level = tracing::Level::WARN;

/// The maximum amount of 4 bytes that a stack can occupy.
/// This stores the amount of 4 bytes that can be used since not having
/// a multiple of four is odd, and can be merely rounded.
#[derive(Debug, Clone)]
pub struct MaxStackSize(NonZeroUsize);
impl MaxStackSize {
    /// Construct a max stack size with the number of 4 bytes that a stack can occupy
    /// Note: If receiving bytes, then likely dividing by 4 and rounding down will work well.
    pub fn new(entries: NonZeroUsize) -> MaxStackSize {
        MaxStackSize(entries)
    }

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

pub(crate) struct State {
    object_id: ClassId,
    entry_point_class: Option<ClassId>,
    entry_point_method: Option<MethodId>,
    conf: StateConfig,
}
impl State {
    fn new(conf: StateConfig, prog: &mut ProgramInfo) -> Self {
        let object_id = prog
            .class_names
            .gcid_from_slice(&["java", "lang", "Object"]);
        Self {
            object_id,
            entry_point_class: None,
            entry_point_method: None,
            conf,
        }
    }
}

fn main() {
    let conf = StateConfig::new();

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open("./rho.log")
        .expect("Expected to be able to open log file");
    let log_file = std::sync::Arc::new(log_file);

    let t_subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(conf.tracing_level)
        .without_time()
        .event_format(formatter::Formatter)
        .with_writer(std::io::stderr.and(log_file))
        .finish();
    // TODO: We may make this jvm a library so this should not be done
    tracing::subscriber::set_global_default(t_subscriber)
        .expect("failed to set global default tracing subscriber");

    info!("RhoJVM Initializing");

    let entry_point_cp = ["HelloWorld"];
    let class_dirs = ["./rhojvm/ex/rt/", "./rhojvm/ex/jce/", "./rhojvm/ex/"];

    // Initialize ProgramInfo
    let mut prog = ProgramInfo::new(Config {
        verify_method_access_flags: true,
    });
    for path in class_dirs.into_iter() {
        let path = Path::new(path);
        prog.class_directories
            .add(path)
            .expect("for class directory to properly exist");
    }

    // Initialize State
    let mut state = State::new(conf, &mut prog);

    // Load the entry point
    let entrypoint_id: ClassFileId = prog
        .class_files
        .load_by_class_path_slice(
            &prog.class_directories,
            &mut prog.class_names,
            &entry_point_cp,
        )
        .unwrap();
    prog.classes
        .load_class(
            &prog.class_directories,
            &mut prog.class_names,
            &mut prog.class_files,
            &mut prog.packages,
            entrypoint_id,
        )
        .unwrap();
    state.entry_point_class = Some(entrypoint_id);

    initialize_class(&mut prog, &mut state, entrypoint_id).unwrap();

    // Run the main method
    let string_id = prog
        .class_names
        .gcid_from_slice(&["java", "lang", "String"]);
    let main_name = "main";
    let main_descriptor = MethodDescriptor::new_void(vec![DescriptorType::single_array(
        DescriptorTypeBasic::Class(string_id),
    )]);
    let main_method_id = prog
        .load_method_from_desc(entrypoint_id, Cow::Borrowed(main_name), &main_descriptor)
        .unwrap();

    state.entry_point_method = Some(main_method_id);
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

#[derive(Debug)]
pub enum VerificationError {
    StackMap(StackMapError),
    VerifyStackMapError(VerifyStackMapError),
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
    prog: &mut ProgramInfo,
    state: &mut State,
    class_id: ClassId,
) -> Result<(), GeneralError> {
    // TODO: Technically we don't have to verify according to the type checking rules
    // for class files < version 50.0
    // and, if the type checking fails for version == 50.0, then we can choose to
    // do verification through type inference

    // - classIsTypeSafe
    // Load super classes
    let mut iter = prog.load_super_classes_iter(class_id);

    // Skip the first class, since that is the base and so it is allowed to be final
    // We store the latest class so that we can update it and use it for errors
    // and checking if the topmost class is Object
    let mut latest_class = iter
        .next_item(
            &prog.class_directories,
            &mut prog.class_names,
            &mut prog.class_files,
            &mut prog.classes,
            &mut prog.packages,
        )
        .expect("The base to be included in the processing")?;

    while let Some(res) = iter.next_item(
        &prog.class_directories,
        &mut prog.class_names,
        &mut prog.class_files,
        &mut prog.classes,
        &mut prog.packages,
    ) {
        let super_class_id = res?;

        // TODO: Are we intended to preinitialize the entire super-chain?
        let class = prog.classes.get(&super_class_id).unwrap();
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

    verify_type_safe_methods(prog, state, class_id)?;

    Ok(())
}

fn verify_type_safe_methods(
    prog: &mut ProgramInfo,
    state: &mut State,
    class_id: ClassId,
) -> Result<(), GeneralError> {
    prog.load_class_from_id(class_id)?;

    let class = prog.classes.get(&class_id).unwrap();
    let method_id_iter = match class {
        ClassVariant::Class(class) => class.iter_method_ids(),
        ClassVariant::Array(_) => {
            tracing::warn!("TODO: Skipped verifying ArrayClass methods");
            return Ok(());
        }
    };

    for method_id in method_id_iter {
        verify_type_safe_method(prog, state, method_id)?;
    }
    Ok(())
}

fn verify_type_safe_method(
    prog: &mut ProgramInfo,
    state: &mut State,
    method_id: MethodId,
) -> Result<(), GeneralError> {
    prog.load_method_from_id(method_id)?;
    prog.verify_method_access_flags(method_id)?;
    // TODO: Document that this assures that it isn't overriding a final method
    prog.init_method_overrides(method_id)?;

    prog.load_method_code(method_id)?;

    let method = prog.methods.get(&method_id).unwrap();
    if method.should_have_code() {
        if method.code().is_none() {
            // We should have code but there was no code!
            return Err(VerificationError::NoMethodCode { method_id }.into());
        } else {
            stack_map::verify_type_safe_method_stack_map(prog, state, method_id)?;
        }
    }

    Ok(())
}

// 5.5
fn initialize_class(
    prog: &mut ProgramInfo,
    state: &mut State,
    class_id: ClassId,
) -> Result<(), GeneralError> {
    pre_initialize_class(prog, state, class_id)?;

    let class = prog.classes.get(&class_id).unwrap().as_class().unwrap();
    for method_id in class.iter_method_ids() {
        prog.load_method_from_id(method_id)?;
        prog.load_method_descriptor_types(method_id)?;
        //     let method = prog.methods.get(&method_id).unwrap();
        //     let param_len = method.descriptor().parameters().len();
        //     for param_i in 0..param_len {
        //         let method = prog.methods.get(&method_id).unwrap();
        //         let param = &method.descriptor().parameters()[param_i];
        //     }
    }

    Ok(())
}

fn run_method_code(
    prog: &mut ProgramInfo,
    state: &mut State,
    method_id: MethodId,
) -> Result<(), GeneralError> {
    prog.load_method_code(method_id)?;

    let inst_count = {
        let method = prog.methods.get(&method_id).unwrap();
        let code = method.code().unwrap();
        code.instructions().len()
    };
    for index in 0..inst_count {
        run_inst(prog, state, method_id, index)?;
    }

    Ok(())
}

/// Assumes that code already exists
fn run_inst(
    prog: &mut ProgramInfo,
    state: &mut State,
    method_id: MethodId,
    inst_index: usize,
) -> Result<(), RunInstError> {
    use rhojvm_base::code::op::GetStatic;
    let (class_id, _) = method_id.decompose();

    let class_file = prog
        .class_files
        .get(&class_id)
        .ok_or(RunInstError::NoClassFile(class_id))?;
    let method = prog
        .methods
        .get(&method_id)
        .ok_or(RunInstError::NoMethod(method_id))?;
    let code = method.code().ok_or(RunInstError::NoCode(method_id))?;

    let (_, inst) = code
        .instructions()
        .get(inst_index)
        .ok_or(RunInstError::NoInst(method_id, inst_index))?
        .clone();
    match inst {
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
