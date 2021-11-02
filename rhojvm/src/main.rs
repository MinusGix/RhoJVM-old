use std::{borrow::Cow, num::NonZeroUsize, path::Path};

use rhojvm_base::{
    code::{
        method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
        op::InstM,
    },
    id::{ClassFileId, ClassId, MethodId},
    Config, ProgramInfo, StepError,
};
use tracing::info;

mod formatter;

const ENV_TRACING_LEVEL: &str = "RHO_LOG_LEVEL";
const DEFAULT_TRACING_LEVEL: tracing::Level = tracing::Level::WARN;
struct StateConfig {
    tracing_level: tracing::Level,
}
impl StateConfig {
    fn new() -> StateConfig {
        let tracing_level = StateConfig::compute_tracing_level();
        StateConfig { tracing_level }
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

struct State {
    object_id: ClassId,
    entry_point_class: Option<ClassId>,
    entry_point_method: Option<MethodId>,
    conf: StateConfig,
}
impl State {
    pub fn new(conf: StateConfig, prog: &mut ProgramInfo) -> Self {
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

    let t_subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(conf.tracing_level)
        .without_time()
        .event_format(formatter::Formatter)
        .finish();
    // TODO: We may make this jvm a library so this should not be done
    tracing::subscriber::set_global_default(t_subscriber)
        .expect("failed to set global default tracing subscriber");

    info!("RhoJVM Initializing");

    let entry_point_cp = ["HelloWorld"];
    let class_dirs = ["./ex/rt/", "./ex/jce/", "./ex/"];

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
enum GeneralError {
    Step(StepError),
    RunInst(RunInstError),
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

#[derive(Debug)]
enum RunInstError {
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
    Ok(())
}

// 5.5
fn initialize_class(
    prog: &mut ProgramInfo,
    state: &mut State,
    class_id: ClassId,
) -> Result<(), GeneralError> {
    prog.load_super_classes(class_id)?;

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
