use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use rhojvm::{
    class_instance::ReferenceArrayInstance,
    eval::{eval_method, EvalMethodValue, Frame, Locals},
    initialize_class,
    jni::native_interface::NativeInterface,
    rv::RuntimeValue,
    util::Env,
    verify_from_entrypoint, State, StateConfig, ThreadData,
};
use rhojvm_base::{
    code::method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
    data::{class_files::ClassFiles, class_names::ClassNames, classes::Classes, methods::Methods},
    id::ClassId,
    package::Packages,
};
use rhojvm_class_loaders::ClassDirectories;
use stack_map_verifier::StackMapVerificationLogging;
use tracing_subscriber::layer::SubscriberExt;

mod formatter;

// TODO: We should provide some separate binary wrapper
//   (or maybe a command line argument? but that isn't as portable to a wide variety of programs)
// that emulates the official jvm's flags
#[derive(Debug, Parser)]
#[clap(name = "RhoJVM (Frontend)")]
#[clap(author = "MinusGix")]
#[clap(version = "0.1.0")]
#[clap(about = "A JVM implementation")]
#[clap(propagate_version = true)]
struct CliArgs {
    #[clap(subcommand)]
    command: CliCommands,
}

#[derive(Debug, Subcommand)]
enum CliCommands {
    Run {
        // Class file name to run
        #[clap(value_name = "CLASS_NAME")]
        class_name: String,
    },
    RunJar {
        #[clap(parse(from_os_str), value_name = "JAR_FILE")]
        jar: PathBuf,
    },
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

    // Note that clap autoexits if it didn't get a thing to do
    let args = CliArgs::parse();

    match &args.command {
        CliCommands::Run { class_name } => execute_class_name(&args, class_name.as_str()),
        CliCommands::RunJar { jar } => todo!("Running a jar is not yet implemented"),
    }
}

fn execute_class_name(args: &CliArgs, class_name: &str) {
    let mut conf = StateConfig::new();
    conf.stack_map_verification_logging = StackMapVerificationLogging {
        log_method_name: false,
        log_received_frame: false,
        log_instruction: false,
        log_stack_modifications: false,
        log_local_variable_modifications: false,
    };

    init_logging(&conf);

    tracing::info!("RhoJVM Initializing");

    let mut class_directories: ClassDirectories = ClassDirectories::default();
    {
        let class_dirs = [
            // RhoJVM libraries take precedence since it is expected that some code
            "./classpath/",
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
    }
    let class_names: ClassNames = ClassNames::default();
    let class_files: ClassFiles = ClassFiles::new(class_directories);
    let classes: Classes = Classes::default();
    let packages: Packages = Packages::default();
    let methods: Methods = Methods::default();

    let entry_point_cp = [class_name];

    // Initialize State
    let state = State::new(conf);

    let main_thread_data = ThreadData::new(std::thread::current().id());

    // The general environment structure
    // This is also used for passing it directly into native functions
    let env = Env {
        interface: Box::leak(Box::new(NativeInterface::new_typical())),
        class_names,
        class_files,
        classes,
        packages,
        methods,
        state,
        tdata: main_thread_data,
    };
    // We pin this, because the env ptr is expected to stay the same
    let mut env = Box::pin(env);
    let mut env: &mut Env = &mut *env;

    // libjava.so depends on libjvm and can't find it itself
    let needed_libs = [
        "./rhojvm/ex/lib/amd64/server/libjvm.so",
        "./rhojvm/ex/lib/amd64/libjava.so",
    ];
    for lib_path in needed_libs {
        unsafe {
            env.state
                .native
                .load_library_blocking(lib_path)
                .expect("Failed to load lib");
        };

        // TODO: Actually call this.
        // TODO: Check for JNI_OnLoadL?
        let _onload = unsafe {
            env.state
                .native
                .find_symbol_blocking_jni_on_load_in_library(lib_path)
        };
        // if let Ok(onload) = onload {
        //     todo!("Call onload function");
        // }
    }

    // Load the entry point
    let entrypoint_id: ClassId = env
        .class_files
        .load_by_class_path_slice(&mut env.class_names, &entry_point_cp)
        .unwrap();
    env.classes
        .load_class(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.packages,
            entrypoint_id,
        )
        .unwrap();
    env.state.entry_point_class = Some(entrypoint_id);

    if let Err(err) = verify_from_entrypoint(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
        &mut env.methods,
        &mut env.state,
        entrypoint_id,
    ) {
        tracing::error!("failed to verify entrypoint class: {:?}", err);
        return;
    }

    if let Err(err) = initialize_class(env, entrypoint_id) {
        tracing::error!("failed to initialize entrypoint class {:?}", err);
        return;
    }

    // We get the main method's id so then we can execute it.
    // We could check this early to make so errors from a missing main show up faster, but that is
    // an edge-case, and doesn't matter.
    {
        let string_id = env.class_names.gcid_from_bytes(b"java/lang/String");
        let main_name = b"main";
        let main_descriptor = MethodDescriptor::new_void(vec![DescriptorType::single_array(
            DescriptorTypeBasic::Class(string_id),
        )]);
        let main_method_id = env
            .methods
            .load_method_from_desc(
                &mut env.class_names,
                &mut env.class_files,
                entrypoint_id,
                main_name,
                &main_descriptor,
            )
            .expect("Failed to load main method");
        let args = {
            let array_id = env
                .class_names
                .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), string_id)
                .expect("Failed to construct type for String[]");
            // TODO: actually construct args
            let array = ReferenceArrayInstance::new(array_id, string_id, Vec::new());
            let array_ref = env.state.gc.alloc(array);
            array_ref.into_generic()
        };
        let frame = Frame::new_locals(Locals::new_with_array([RuntimeValue::Reference(args)]));
        match eval_method(env, main_method_id, frame) {
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
