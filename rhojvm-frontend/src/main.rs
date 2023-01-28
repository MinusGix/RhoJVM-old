use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    pin::Pin,
};

use clap::{Parser, Subcommand};

use rhojvm::{
    class_instance::{ClassInstance, ReferenceArrayInstance, ThreadInstance},
    eval::{
        eval_method, instances::make_instance_fields, EvalMethodValue, Frame, Locals,
        ValueException,
    },
    gc::GcRef,
    initialize_class,
    jni::native_interface::NativeInterface,
    rv::RuntimeValue,
    string_intern::StringInterner,
    util::{get_string_contents_as_rust_string, Env},
    verify_from_entrypoint, GeneralError, State, StateConfig, ThreadData,
};
use rhojvm_base::{
    code::method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
    data::{
        class_file_loader::ClassFileLoader, class_files::ClassFiles, class_names::ClassNames,
        classes::Classes, methods::Methods,
    },
    id::ClassId,
    package::Packages,
};
use rhojvm_class_loaders::{jar_loader::JarClassFileLoader, util::CombineLoader, ClassDirectories};
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
impl CliArgs {
    pub fn abort_on_unsupported(&self) -> bool {
        match &self.command {
            CliCommands::Run {
                abort_on_unsupported,
                ..
            } => *abort_on_unsupported,
            CliCommands::RunJar {
                abort_on_unsupported,
                ..
            } => *abort_on_unsupported,
        }
    }

    pub fn log_class_names(&self) -> bool {
        match &self.command {
            CliCommands::Run {
                log_class_names, ..
            } => *log_class_names,
            CliCommands::RunJar {
                log_class_names, ..
            } => *log_class_names,
        }
    }
}

#[derive(Debug, Subcommand)]
enum CliCommands {
    Run {
        // Class file name to run
        #[clap(value_name = "CLASS_NAME")]
        class_name: String,
        // TODO: Can we avoid duplication?
        #[clap(long)]
        abort_on_unsupported: bool,
        #[clap(long)]
        log_class_names: bool,
    },
    RunJar {
        #[clap(parse(from_os_str), value_name = "JAR_FILE")]
        jar: PathBuf,
        #[clap(long)]
        abort_on_unsupported: bool,
        #[clap(long)]
        log_class_names: bool,
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
    let conf = make_state_conf(&args);

    match &args.command {
        CliCommands::Run { class_name, .. } => {
            let class_directories = make_class_directories(&args);
            // TODO: This is probably incorrect
            execute_class_name(
                &args,
                conf,
                class_directories,
                |env| {
                    env.class_files
                        .load_by_class_path_slice(&mut env.class_names, &[class_name.as_str()])
                        .unwrap()
                },
                |_| {},
            )
        }
        CliCommands::RunJar { jar, .. } => {
            execute_jar(&args, conf, jar);
        }
    }
}

fn execute_jar(args: &CliArgs, conf: StateConfig, jar: &PathBuf) {
    let class_directories = make_class_directories(args);
    let mut jar_loader = JarClassFileLoader::new(jar.to_owned()).expect("Failed to load jar file");

    let manifest = jar_loader
        .load_manifest()
        .expect("Failed to load jar manifest");
    // TODO: Check manifest version
    // TODO: Add manifest class path field to class directories
    // TODO: manifest extensions?
    // TODO: manifest implementation field?
    // TODO: manifest sealed?

    let root_index = 0;
    // This will be dot separated
    let main_class_name = manifest
        .get(root_index, "Main-Class")
        .expect("There was no Main-Class manifest attribute");
    // Convert it to bytes since we typically deal with cesu8
    // TODO: actually convert it to cesu8
    let main_class_name = main_class_name.as_bytes();
    let main_class_name = main_class_name.split(|x| *x == b'.');
    let main_class_name = &main_class_name;

    // Combine the jar loader and the class directories loader into one loader
    let combine_loader = CombineLoader::new(jar_loader, class_directories);

    execute_class_name(
        args,
        conf,
        combine_loader,
        |env| {
            env.class_names
                .gcid_from_iter_bytes(main_class_name.clone())
        },
        |env| {
            let main_id = env
                .class_names
                .gcid_from_iter_bytes(main_class_name.clone());
            let package_id = env
                .classes
                .get(&main_id)
                .expect("class should be instantiated at pre-execute stage")
                .package();

            let info = if let Some(package_id) = package_id {
                &mut env
                    .packages
                    .get_mut(package_id)
                    .expect("package id was not valid in pre-execute stage")
                    .info
            } else {
                &mut env.packages.null_package_info
            };

            if let Some(spec_title) = manifest.get(root_index, "Specification-Title") {
                info.specification_title = Some(spec_title.to_string());
            }

            if let Some(spec_vendor) = manifest.get(root_index, "Specification-Vendor") {
                info.specification_vendor = Some(spec_vendor.to_string());
            }

            if let Some(spec_version) = manifest.get(root_index, "Specification-Version") {
                info.specification_version = Some(spec_version.to_string());
            }

            if let Some(impl_title) = manifest.get(root_index, "Implementation-Title") {
                info.implementation_title = Some(impl_title.to_string());
            }

            if let Some(impl_vendor) = manifest.get(root_index, "Implementation-Vendor") {
                info.implementation_vendor = Some(impl_vendor.to_string());
            }

            if let Some(impl_version) = manifest.get(root_index, "Implementation-Version") {
                info.implementation_version = Some(impl_version.to_string());
            }

            if let Some(sealed) = manifest.get(root_index, "Sealed") {
                if sealed == "true" {
                    info.sealed = Some(true);
                } else if sealed == "false" {
                    info.sealed = Some(false);
                } else {
                    tracing::warn!("Manifest's sealed property was not true or false. Ignoring.");
                }
            }
        },
    );
}

fn make_state_conf(args: &CliArgs) -> StateConfig {
    // TODO: make arguments for these
    let mut conf = StateConfig::new();
    conf.stack_map_verification_logging = StackMapVerificationLogging {
        log_method_name: false,
        log_received_frame: false,
        log_instruction: false,
        log_stack_modifications: false,
        log_local_variable_modifications: false,
    };
    conf.abort_on_unsupported = args.abort_on_unsupported();
    conf.log_class_names = args.log_class_names();
    conf
}

fn make_class_directories(_args: &CliArgs) -> ClassDirectories {
    // TODO: accept class directories from command line
    let mut class_directories: ClassDirectories = ClassDirectories::default();

    let class_dirs = [
        // RhoJVM libraries take precedence since we need to implement internal versions
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

    class_directories
}

fn make_env(
    _args: &CliArgs,
    conf: StateConfig,
    cfile_loader: impl ClassFileLoader + 'static,
) -> Pin<Box<Env>> {
    tracing::info!("RhoJVM Initializing");

    let class_names: ClassNames = ClassNames::default();
    let class_files: ClassFiles = ClassFiles::new(cfile_loader);
    let classes: Classes = Classes::default();
    let packages: Packages = Packages::default();
    let methods: Methods = Methods::default();

    // Initialize State
    let state = State::new(conf);

    let main_thread_data = ThreadData::new(std::thread::current().id());

    // The general environment structure
    // This is also used for passing it directly into native functions
    let env = Env::new(
        Box::leak(Box::new(NativeInterface::new_typical())),
        class_names,
        class_files,
        classes,
        packages,
        methods,
        state,
        main_thread_data,
        StringInterner::default(),
    );
    // We pin this, because the env ptr is expected to stay the same
    Box::pin(env)
}

fn load_required_libs(env: &mut Env) {
    // libjava.so depends on libjvm and can't find it itself
    let needed_libs = [
        "./rhojvm/ex/lib/amd64/server/libjvm.so",
        "./rhojvm/ex/lib/amd64/libjava.so",
        "./rhojvm/ex/lib/amd64/libzip.so",
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
}

fn execute_class_name(
    args: &CliArgs,
    conf: StateConfig,
    cfile_loader: impl ClassFileLoader + 'static,
    get_entrypoint_id: impl FnOnce(&mut Env) -> ClassId,
    pre_execute: impl FnOnce(&mut Env),
) {
    init_logging(&conf);

    let mut env = make_env(args, conf, cfile_loader);
    let mut env: &mut Env = &mut *env;

    load_required_libs(env);

    // Initialize the thread reference
    let main_thread_thread_ref = {
        let thread_class_id = env.class_names.gcid_from_bytes(b"java/lang/Thread");
        let thread_static_ref = initialize_class(env, thread_class_id).unwrap().into_value();
        let thread_static_ref = match thread_static_ref {
            ValueException::Value(re) => re,
            ValueException::Exception(_) => panic!("Exception initializing main thread"),
        };

        let fields = make_instance_fields(env, thread_class_id).unwrap();
        let ValueException::Value(fields) = fields else {
            panic!("Exception initializing main thread. Failed to create fields");
        };

        // new does not run a constructor, it only initializes it
        let class = ClassInstance {
            instanceof: thread_class_id,
            static_ref: thread_static_ref,
            fields,
        };

        let thread_class = ThreadInstance::new(class, Some(env.tdata.id));
        env.state.gc.alloc(thread_class)
    };

    env.tdata.thread_instance = Some(main_thread_thread_ref);

    let entrypoint_id = get_entrypoint_id(env);
    // Load the entry point
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

        pre_execute(env);

        match eval_method(env, main_method_id.into(), frame) {
            Ok(res) => match res {
                EvalMethodValue::ReturnVoid => (),
                EvalMethodValue::Return(v) => {
                    tracing::warn!("Main returned a value: {:?}", v);
                }
                // TODO: Call the method to get a string from the exception
                EvalMethodValue::Exception(exc) => {
                    eprintln!("There was an unhandled exception.");
                    let text = to_string_for(env, exc)
                        .expect("Got an internal error when calling toString on exception.");
                    match text {
                        ValueException::Value(text) => eprintln!("{}", text),
                        // TODO: We could provide some info.
                        ValueException::Exception(_) => eprintln!(
                            "There was an exception calling toString on the thrown exception. Skipping trying to call toString on that exception.."
                        ),
                    }
                }
            },
            Err(err) => {
                tracing::error!("There was an error in running the method: {:?}", err);
                eprintln!("There was an internal error in running code: {:?}", err);
            }
        }

        if env.state.conf.log_class_names {
            tracing::info!("Class Names: {:#?}", env.class_names);
        }
    }
}

fn to_string_for(
    env: &mut Env,
    val: GcRef<ClassInstance>,
) -> Result<ValueException<String>, GeneralError> {
    let string_id = env.class_names.gcid_from_bytes(b"java/lang/String");
    let throwable_id = env.class_names.gcid_from_bytes(b"java/lang/Throwable");
    let to_string_desc =
        MethodDescriptor::new_ret(DescriptorType::Basic(DescriptorTypeBasic::Class(string_id)));

    let to_string_id = env.methods.load_method_from_desc(
        &mut env.class_names,
        &mut env.class_files,
        throwable_id,
        b"toString",
        &to_string_desc,
    )?;

    let text = eval_method(
        env,
        to_string_id.into(),
        Frame::new_locals(Locals::new_with_array([RuntimeValue::Reference(
            val.into_generic(),
        )])),
    )?;
    let text = match text {
        EvalMethodValue::ReturnVoid => {
            // TODO: don't panic
            panic!("We got nothing from calling toString");
        }
        EvalMethodValue::Return(text) => text,
        EvalMethodValue::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    let text = if let RuntimeValue::Reference(text) = text {
        text
    } else {
        panic!("Text was not a reference and thus could not be a string");
    };

    get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        text.into_generic(),
    )
    .map(ValueException::Value)
}
