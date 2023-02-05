use std::borrow::Cow;

use indexmap::IndexMap;
use rhojvm_base::code::{
    method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
    types::JavaChar,
};
use sysinfo::SystemExt;
use usize_cast::IntoUsize;

use crate::{
    class_instance::{Instance, PrimitiveArrayInstance, ReferenceArrayInstance, ReferenceInstance},
    eval::{eval_method, Frame, Locals, ValueException},
    gc::GcRef,
    jni::{JClass, JInt, JLong, JObject, JString},
    rv::{RuntimeValue, RuntimeValuePrimitive},
    util::{
        construct_string, construct_string_r, get_string_contents_as_rust_string, ref_info, Env,
    },
    StateConfig,
};

/// Initialize properties based on operating system
#[allow(clippy::items_after_statements)]
pub(crate) extern "C" fn system_set_properties(env: *mut Env<'_>, _this: JObject, props: JObject) {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let props = unsafe { env.get_jobject_as_gcref(props) };
    let props = props.expect("internal errror: System properties was null");
    let props = props.unchecked_as();

    let properties_id = env.class_names.gcid_from_bytes(b"java/util/Properties");
    let object_id = env.class_names.object_id();
    let string_id = env.state.string_class_id(&mut env.class_names);
    // Object setProperty(String key, String value)
    let set_property_desc = MethodDescriptor::new(
        smallvec::smallvec![
            DescriptorType::Basic(DescriptorTypeBasic::Class(string_id)),
            DescriptorType::Basic(DescriptorTypeBasic::Class(string_id)),
        ],
        Some(DescriptorType::Basic(DescriptorTypeBasic::Class(object_id))),
    );

    let set_property_id = env
        .methods
        .load_method_from_desc(
            &mut env.class_names,
            &mut env.class_files,
            properties_id,
            b"setProperty",
            &set_property_desc,
        )
        .expect("Failed to load Properties#setProperty method which is needed for initialization");

    let properties = Properties::get_properties(env);

    // TODO: We could lessen the work a bit by writing as ascii
    // or directly as utf-16
    for (property_name, property_value) in properties.into_iter() {
        let property_name = property_name
            .encode_utf16()
            .map(|x| RuntimeValuePrimitive::Char(JavaChar(x)))
            .collect();
        let property_name = match construct_string(env, property_name)
            .expect("Failed to construct UTF-16 string for property name")
        {
            ValueException::Value(name) => name,
            ValueException::Exception(_) => {
                panic!("There was an exception constructing a UTF-16 string for property name")
            }
        };

        let property_value = property_value
            .encode_utf16()
            .map(|x| RuntimeValuePrimitive::Char(JavaChar(x)))
            .collect();
        let property_value = match construct_string(env, property_value)
            .expect("Failed to construct UTF-16 string for property value")
        {
            ValueException::Value(name) => name,
            ValueException::Exception(_) => {
                panic!("There was an exception constructing a UTF-16 string for property value")
            }
        };

        let frame = Frame::new_locals(Locals::new_with_array([
            RuntimeValue::Reference(props),
            RuntimeValue::Reference(property_name.into_generic()),
            RuntimeValue::Reference(property_value.into_generic()),
        ]));
        eval_method(env, set_property_id.into(), frame).expect("Failed to set property");
    }
}

const RUNTIME_NAME: &str = "RhoJVM";
const JNU_ENCODING: &str = "UTF-8";
const FILE_ENCODING: &str = "UTF-8";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const VENDOR: &str = "RhoJVM Contributors";
const VENDOR_URL: &str = "https://github.com/MinusGix/rhojvm";
const VENDOR_URL_BUG: &str = "https://github.com/MinusGix/rhojvm/issues";
// TODO: Include architecture it was compiled for?
const VM_NAME: &str = "RhoJVM";

// Disallow dead code so that no properties are ignored!
// This guards against us adding a property but forgetting to add it to the iterator which will
// initialize them all
#[deny(dead_code)]
struct Properties {
    runtime_name: &'static str,

    file_sep: &'static str,
    line_sep: &'static str,
    /// Separate paths in a list
    path_sep: &'static str,
    file_encoding: &'static str,
    jnu_encoding: &'static str,
    os_name: Cow<'static, str>,
    os_arch: &'static str,
    os_version: Cow<'static, str>,

    user_dir: Cow<'static, str>,

    tmpdir: Cow<'static, str>,

    username: Cow<'static, str>,
    user_home: Cow<'static, str>,
    user_language: Cow<'static, str>,
    java_library_path: Cow<'static, str>,

    java_vm_version: &'static str,
    java_vm_vendor: &'static str,
    java_vendor_url: &'static str,
    java_vendor_url_bug: &'static str,
    java_vm_name: &'static str,
    java_home: Cow<'static, str>,

    extra: IndexMap<String, String>,
}
impl Properties {
    // TODO: Can we warn/error at compile time if there is unknown data?
    fn get_properties(env: &mut Env) -> Properties {
        // TODO: Is line sep correct?
        if cfg!(target_os = "windows") || cfg!(target_family = "windows") {
            Properties::windows_properties(&env.state.conf, &env.system_info)
        } else if cfg!(unix) {
            // FIXME: Provide more detailed names and information
            // for MacOS
            Properties::unix_properties(&env.state.conf, &env.system_info)
        } else {
            tracing::warn!("No target os/family detected, assuming unix");
            Properties::unix_properties(&env.state.conf, &env.system_info)
        }
    }

    // CLippy gets a bit confused, it seems
    #[allow(clippy::same_functions_in_if_condition)]
    fn os_arch() -> &'static str {
        if cfg!(target_arch = "x86") {
            "x86"
        } else if cfg!(target_arch = "x86_64") {
            // "x86_64"
            "amd64"
        } else if cfg!(target_arch = "mips") {
            // TODO: Correct?
            "mips"
        } else if cfg!(target_arch = "powerpc") || cfg!(target_arch = "powerpc64") {
            // TODO: correct?
            "ppc"
        } else if cfg!(target_arch = "arm") || cfg!(target_arch = "aarch64") {
            // TODO: correct?
            "arm"
        } else if cfg!(target_arch = "riscv") {
            "rsicv"
        } else if cfg!(target_arch = "wasm32") {
            "wasm32"
        } else {
            tracing::warn!("Unknown architecture");
            "unknown"
        }
    }

    fn windows_properties(conf: &StateConfig, sys: &sysinfo::System) -> Properties {
        Properties {
            runtime_name: RUNTIME_NAME,
            file_sep: "\\",
            line_sep: "\n",
            path_sep: ";",
            file_encoding: FILE_ENCODING,
            jnu_encoding: JNU_ENCODING,
            os_name: Cow::Borrowed("Windows"),
            os_version: sys
                .kernel_version()
                .map(|x| Cow::Owned(x.to_string()))
                .unwrap_or(Cow::Borrowed("Unknown")),
            os_arch: Properties::os_arch(),
            // TODO: Can we do better?
            user_dir: std::env::current_dir()
                .ok()
                .and_then(|x| x.to_str().map(ToString::to_string))
                .map_or(Cow::Borrowed("C:\\"), Cow::Owned),
            tmpdir: std::env::temp_dir()
                .to_str()
                .map_or(Cow::Borrowed("/tmp"), |x| Cow::Owned(x.to_string())),
            username: Cow::Owned(whoami::username()),
            user_home: dirs::home_dir()
                .and_then(|x| x.to_str().map(ToString::to_string))
                .map_or(Cow::Borrowed("?"), Cow::Owned),
            // TODO: Detect this from system? Provide a setting to override it?
            user_language: Cow::Borrowed("en"),
            // TODO: Give a good value?
            java_library_path: Cow::Borrowed(""),
            // TODO: Typically the java.vm.version/java.runtime.version have more information
            // such as the build date
            java_vm_version: VERSION,
            java_vm_vendor: VENDOR,
            java_vendor_url: VENDOR_URL,
            java_vendor_url_bug: VENDOR_URL_BUG,
            java_vm_name: VM_NAME,
            java_home: Cow::Owned(conf.java_home.clone()),
            extra: conf.properties.clone(),
        }
    }

    fn unix_properties(conf: &StateConfig, sys: &sysinfo::System) -> Properties {
        Properties {
            runtime_name: RUNTIME_NAME,
            file_sep: "/",
            line_sep: "\n",
            path_sep: ":",
            file_encoding: FILE_ENCODING,
            jnu_encoding: JNU_ENCODING,
            os_name: Cow::Owned(whoami::platform().to_string()),
            os_version: sys
                .kernel_version()
                .map_or(Cow::Borrowed("Unknown"), Cow::Owned),
            os_arch: Properties::os_arch(),
            // TODO: Can we do better?
            user_dir: std::env::current_dir()
                .ok()
                .and_then(|x| x.to_str().map(ToString::to_string))
                .map_or(Cow::Borrowed("/"), Cow::Owned),
            tmpdir: std::env::temp_dir()
                .to_str()
                .map_or(Cow::Borrowed("/tmp"), |x| Cow::Owned(x.to_string())),
            username: Cow::Owned(whoami::username()),
            user_home: dirs::home_dir()
                .and_then(|x| x.to_str().map(ToString::to_string))
                .map_or(Cow::Borrowed("?"), Cow::Owned),
            // TODO: Detect this from system? Provide a setting to override it?
            user_language: Cow::Borrowed("en"),
            // TODO: Give a good value?
            java_library_path: Cow::Borrowed(""),
            java_vm_version: VERSION,
            java_vm_vendor: VENDOR,
            java_vendor_url: VENDOR_URL,
            java_vendor_url_bug: VENDOR_URL_BUG,
            java_vm_name: VM_NAME,
            java_home: Cow::Owned(conf.java_home.clone()),
            extra: conf.properties.clone(),
        }
    }

    fn into_iter(self) -> impl Iterator<Item = (Cow<'static, str>, Cow<'static, str>)> {
        [
            (
                Cow::Borrowed("java.runtime.name"),
                Cow::Borrowed(self.runtime_name),
            ),
            (
                Cow::Borrowed("file.separator"),
                Cow::Borrowed(self.file_sep),
            ),
            (
                Cow::Borrowed("line.separator"),
                Cow::Borrowed(self.line_sep),
            ),
            (
                Cow::Borrowed("path.separator"),
                Cow::Borrowed(self.path_sep),
            ),
            (
                Cow::Borrowed("file.encoding"),
                Cow::Borrowed(self.file_encoding),
            ),
            (
                Cow::Borrowed("sun.jnu.encoding"),
                Cow::Borrowed(self.jnu_encoding),
            ),
            (Cow::Borrowed("os.name"), self.os_name),
            (Cow::Borrowed("os.version"), self.os_version),
            (Cow::Borrowed("os.arch"), Cow::Borrowed(self.os_arch)),
            (Cow::Borrowed("user.dir"), self.user_dir),
            (Cow::Borrowed("java.io.tmpdir"), self.tmpdir),
            (Cow::Borrowed("user.name"), self.username),
            (Cow::Borrowed("user.home"), self.user_home),
            (Cow::Borrowed("user.language"), self.user_language),
            (Cow::Borrowed("java.library.path"), self.java_library_path),
            (
                Cow::Borrowed("java.vm.version"),
                Cow::Borrowed(self.java_vm_version),
            ),
            (
                Cow::Borrowed("java.vm.vendor"),
                Cow::Borrowed(self.java_vm_vendor),
            ),
            (
                Cow::Borrowed("java.vendor"),
                Cow::Borrowed(self.java_vm_vendor),
            ),
            (
                Cow::Borrowed("java.vendor.url"),
                Cow::Borrowed(self.java_vendor_url),
            ),
            (
                Cow::Borrowed("java.vendor.url.bug"),
                Cow::Borrowed(self.java_vendor_url_bug),
            ),
            (
                Cow::Borrowed("java.vm.name"),
                Cow::Borrowed(self.java_vm_name),
            ),
            (
                Cow::Borrowed("java.runtime.version"),
                Cow::Borrowed(self.java_vm_version),
            ),
            (
                Cow::Borrowed("java.version"),
                Cow::Borrowed(self.java_vm_version),
            ),
            (Cow::Borrowed("java.home"), self.java_home),
        ]
        .into_iter()
        .chain(
            self.extra
                .into_iter()
                .map(|(k, v)| (Cow::Owned(k), Cow::Owned(v))),
        )
    }
}

pub(crate) extern "C" fn system_load(env: *mut Env<'_>, _: JClass, path: JString) {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let path = unsafe { env.get_jobject_as_gcref(path) };
    let path = path.expect("NPE");

    let path = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        path,
    )
    .unwrap();

    // Safety: We have to trust the java code to not load arbitrary code that is bad for our health
    unsafe {
        env.state
            .native
            .load_library_blocking(path)
            .expect("Failed to load native library");
    }
}

pub(crate) extern "C" fn system_load_library(env: *mut Env<'_>, _: JClass, path: JString) {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let path = unsafe { env.get_jobject_as_gcref(path) };
    let path = path.expect("NPE");

    let path = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        path,
    )
    .unwrap();

    // Safety: We have to trust the java code to not load arbitrary code that is bad for our health
    let res = unsafe { env.state.native.load_library_blocking(&path) };

    if let Err(_) = res {
        let path = {
            if cfg!(target_os = "windows") && !path.ends_with(".dll") {
                format!("{}.dll", path)
            } else if cfg!(target_os = "macos") && !path.ends_with(".dylib") {
                format!("lib{}.dylib", path)
            } else if cfg!(target_family = "unix") && !path.ends_with(".so") {
                format!("lib{}.so", path)
            } else {
                tracing::warn!("Unsure what suffix for libraries to use for this platform");
                path.clone()
            }
        };

        // Try to load `lib{name}` instead
        let libpath = format!("lib{}", path);
        let res = unsafe { env.state.native.load_library_blocking(&libpath) };

        if let Err(_) = res {
            // Prefix it with each of the paths
            for folder in env.state.conf.native_lib_dirs.iter() {
                // `{folder}/{name}`
                let folder_path = format!("{}/{}", folder, path);
                if let Err(_) = unsafe { env.state.native.load_library_blocking(&folder_path) } {
                    // It failed, try loading `{folder}/lib{name}` instead
                    let path = format!("{}/lib{}", folder, path);
                    if let Err(_) = unsafe { env.state.native.load_library_blocking(&path) } {
                        continue;
                    } else {
                        // It succeeded, we're done
                        return;
                    }
                } else {
                    // It succeeded, we're done
                    return;
                }
            }
        } else {
            return;
        }
    } else {
        return;
    }

    // If we got here, we failed to load the library
    panic!("Failed to load native library: {}", path);
}

pub(crate) extern "C" fn system_map_library_name(
    env: *mut Env<'_>,
    _this: JObject,
    name: JString,
) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let name = unsafe { env.get_jobject_as_gcref(name) };
    let name = name.expect("NPE");

    let name = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        name,
    )
    .unwrap();

    tracing::info!("Mapping library name: {}", name);

    let name = {
        if cfg!(target_os = "windows") && !name.ends_with(".dll") {
            format!("{}.dll", name)
        } else if cfg!(target_os = "macos") && !name.ends_with(".dylib") {
            format!("lib{}.dylib", name)
        } else if cfg!(target_family = "unix") && !name.ends_with(".so") {
            format!("lib{}.so", name)
        } else {
            tracing::warn!("Unsure what suffix for libraries to use for this platform");
            name.clone()
        }
    };

    let name = construct_string_r(env, &name).unwrap();
    let Some(name) = env.state.extract_value(name) else {
        return JObject::null();
    };

    unsafe { env.get_local_jobject_for(name.into_generic()) }
}

/// java/lang/System
/// `public static void arraycopy(Object src, int src_pos, Object dest, int dest_position, int
/// length)`
pub(crate) extern "C" fn system_arraycopy(
    env: *mut Env<'_>,
    _this: JObject,
    source: JObject,
    source_start: JInt,
    destination: JObject,
    destination_start: JInt,
    count: JInt,
) {
    assert!(
        !env.is_null(),
        "System arraycopy got a null env, this is indicative of an internal bug."
    );

    let env = unsafe { &mut *env };

    let source_ref = unsafe { env.get_jobject_as_gcref(source) };
    let source_ref = source_ref.expect("null pointer");

    let destination_ref = unsafe { env.get_jobject_as_gcref(destination) };
    let destination_ref = destination_ref.expect("null pointer");

    let source_inst = env.state.gc.deref(source_ref).unwrap();
    let destination_inst = env.state.gc.deref(destination_ref).unwrap();
    match (source_inst, destination_inst) {
        (_, Instance::StaticClass(_)) | (Instance::StaticClass(_), _) => {
            panic!("Should not be a static class")
        }
        (Instance::Reference(src), Instance::Reference(dest)) => match (dest, src) {
            (ReferenceInstance::PrimitiveArray(_), ReferenceInstance::PrimitiveArray(_)) => {
                system_arraycopy_primitive(
                    env,
                    source_ref.unchecked_as::<PrimitiveArrayInstance>(),
                    source_start,
                    destination_ref.unchecked_as::<PrimitiveArrayInstance>(),
                    destination_start,
                    count,
                );
            }
            (ReferenceInstance::ReferenceArray(_), ReferenceInstance::ReferenceArray(_)) => {
                system_arraycopy_references(
                    env,
                    source_ref.unchecked_as::<ReferenceArrayInstance>(),
                    source_start,
                    destination_ref.unchecked_as::<ReferenceArrayInstance>(),
                    destination_start,
                    count,
                );
            }
            (ReferenceInstance::PrimitiveArray(_), _)
            | (_, ReferenceInstance::PrimitiveArray(_)) => {
                todo!(
                    "Wrong types:\nsrc: {}\ndest: {}",
                    ref_info(env, Some(source_ref)),
                    ref_info(env, Some(destination_ref))
                )
            }
            (ReferenceInstance::ReferenceArray(_), _)
            | (_, ReferenceInstance::ReferenceArray(_)) => {
                todo!(
                    "Wrong types:\nsrc: {}\ndest: {}",
                    ref_info(env, Some(source_ref)),
                    ref_info(env, Some(destination_ref))
                )
            }
            _ => panic!("Throw exception, this should be an array"),
        },
    };
}

fn system_arraycopy_references(
    env: &mut Env,
    source_ref: GcRef<ReferenceArrayInstance>,
    source_start: i32,
    destination_ref: GcRef<ReferenceArrayInstance>,
    destination_start: i32,
    count: i32,
) {
    if source_start < 0 || destination_start < 0 {
        todo!("One of the starts was negative");
    } else if count < 0 {
        todo!("Count was an negative");
    }

    let source_start = source_start.unsigned_abs().into_usize();
    let destination_start = destination_start.unsigned_abs().into_usize();
    let count = count.unsigned_abs().into_usize();

    // TODO: We should only need to clone if source == destination!
    let source = env.state.gc.deref(source_ref).unwrap().clone();

    let destination = env.state.gc.deref_mut(destination_ref).unwrap();
    let source_id = source.element_type;
    let dest_id = destination.element_type;

    let is_castable = source_id == dest_id
        || env
            .classes
            .is_super_class(
                &mut env.class_names,
                &mut env.class_files,
                &mut env.packages,
                source_id,
                dest_id,
            )
            .unwrap()
        || env
            .classes
            .implements_interface(
                &mut env.class_names,
                &mut env.class_files,
                source_id,
                dest_id,
            )
            .unwrap()
        || env
            .classes
            .is_castable_array(
                &mut env.class_names,
                &mut env.class_files,
                &mut env.packages,
                source_id,
                dest_id,
            )
            .unwrap();

    if !is_castable {
        todo!("Error about incompatible types")
    }

    // TODO: overflow checks
    let source_end = source_start + count;
    let destination_end = destination_start + count;

    let source_slice = if let Some(source_slice) = source.elements.get(source_start..source_end) {
        source_slice
    } else {
        todo!("Exception about source start exceeding length");
    };

    let destination_slice = if let Some(destination_slice) = destination
        .elements
        .get_mut(destination_start..destination_end)
    {
        destination_slice
    } else {
        todo!("Exception about destination start exceeding length");
    };

    assert_eq!(source_slice.len(), destination_slice.len());

    for (dest, src) in destination_slice.iter_mut().zip(source_slice.iter()) {
        *dest = *src;
    }
}

fn system_arraycopy_primitive(
    env: &mut Env,
    source_ref: GcRef<PrimitiveArrayInstance>,
    source_start: i32,
    destination_ref: GcRef<PrimitiveArrayInstance>,
    destination_start: i32,
    count: i32,
) {
    if source_start < 0 || destination_start < 0 {
        todo!("One of the starts was negative");
    } else if count < 0 {
        todo!("Count was an negative");
    }

    let source_start = source_start.unsigned_abs().into_usize();
    let destination_start = destination_start.unsigned_abs().into_usize();
    let count = count.unsigned_abs().into_usize();

    // TODO: We should only need to clone if source == destination!
    let source = env.state.gc.deref(source_ref).unwrap().clone();

    let destination = env.state.gc.deref_mut(destination_ref).unwrap();

    if source.element_type != destination.element_type {
        todo!("Error about incompatible types")
    }

    // TODO: overflow checks
    let source_end = source_start + count;
    let destination_end = destination_start + count;

    let source_slice = if let Some(source_slice) = source.elements.get(source_start..source_end) {
        source_slice
    } else {
        todo!("Exception about source start exceeding length");
    };

    let destination_slice = if let Some(destination_slice) = destination
        .elements
        .get_mut(destination_start..destination_end)
    {
        destination_slice
    } else {
        todo!("Exception about destination start exceeding length");
    };

    assert_eq!(source_slice.len(), destination_slice.len());

    for (dest, src) in destination_slice.iter_mut().zip(source_slice.iter()) {
        *dest = *src;
    }
}

// We know it might truncate, which is an accepted part of the java api
#[allow(clippy::cast_possible_truncation)]
pub(crate) extern "C" fn system_current_time_milliseconds(env: *mut Env, _: JClass) -> JLong {
    assert!(!env.is_null(), "Null env. Internal bug?");
    let env = unsafe { &mut *env };
    let duration = env.startup_instant.elapsed();
    duration.as_millis() as i64
}

// We know it might truncate, which is an accepted part of the java api
#[allow(clippy::cast_possible_truncation)]
pub(crate) extern "C" fn system_nano_time(env: *mut Env, _: JClass) -> JLong {
    assert!(!env.is_null(), "Null env. Internal bug?");
    let env = unsafe { &mut *env };
    let duration = env.startup_instant.elapsed();
    duration.as_nanos() as i64
}
