use std::borrow::Cow;

use rhojvm_base::code::{
    method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
    types::JavaChar,
};
use sysinfo::{RefreshKind, SystemExt};
use usize_cast::IntoUsize;

use crate::{
    class_instance::{Instance, PrimitiveArrayInstance, ReferenceArrayInstance, ReferenceInstance},
    eval::{eval_method, Frame, Locals, ValueException},
    gc::GcRef,
    jni::{JClass, JInt, JLong, JObject},
    rv::{RuntimeValue, RuntimeValuePrimitive},
    util::{construct_string, Env},
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
    for (property_name, property_value) in properties {
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

struct Properties {
    file_sep: &'static str,
    line_sep: &'static str,
    /// Separate paths in a list
    path_sep: &'static str,
    file_encoding: &'static str,
    os_name: Cow<'static, str>,
    os_arch: &'static str,
}
impl Properties {
    // TODO: Can we warn/error at compile time if there is unknown data?
    fn get_properties(env: &mut Env) -> Properties {
        // TODO: Is line sep correct?
        if cfg!(target_os = "windows") || cfg!(target_family = "windows") {
            Properties::windows_properties()
        } else if cfg!(unix) {
            // FIXME: Provide more detailed names and information
            // for MacOS
            Properties::unix_properties(&env.system_info)
        } else {
            tracing::warn!("No target os/family detected, assuming unix");
            Properties::unix_properties(&env.system_info)
        }
    }

    // CLippy gets a bit confused, it seems
    #[allow(clippy::same_functions_in_if_condition)]
    fn os_arch() -> &'static str {
        if cfg!(target_arch = "x86") {
            "x86"
        } else if cfg!(target_arch = "x86_64") {
            "x86_64"
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

    fn windows_properties() -> Properties {
        Properties {
            file_sep: "\\",
            line_sep: "\n",
            path_sep: ";",
            file_encoding: "UTF-8",
            os_name: Cow::Borrowed("Windows"),
            os_arch: Properties::os_arch(),
        }
    }

    fn unix_properties(sys: &sysinfo::System) -> Properties {
        Properties {
            file_sep: "/",
            line_sep: "\n",
            path_sep: ":",
            file_encoding: "UTF-8",
            os_name: sys
                .long_os_version()
                .map_or(Cow::Borrowed("Unix"), Cow::Owned),
            os_arch: Properties::os_arch(),
        }
    }
}
impl IntoIterator for Properties {
    type Item = (&'static str, Cow<'static, str>);

    type IntoIter = std::array::IntoIter<Self::Item, 6>;

    fn into_iter(self) -> Self::IntoIter {
        // TODO: Could we provide a compile error if we don't use all the fields?
        [
            ("file.separator", Cow::Borrowed(self.file_sep)),
            ("line.separator", Cow::Borrowed(self.line_sep)),
            ("path.separator", Cow::Borrowed(self.path_sep)),
            ("file.encoding", Cow::Borrowed(self.file_encoding)),
            ("os.name", self.os_name),
            ("os.arch", Cow::Borrowed(self.os_arch)),
        ]
        .into_iter()
    }
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
            | (_, ReferenceInstance::PrimitiveArray(_)) => todo!("Wrong types"),
            (ReferenceInstance::ReferenceArray(_), _)
            | (_, ReferenceInstance::ReferenceArray(_)) => todo!("Wrong types"),
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
