use rhojvm_base::{
    package::Packages, util::MemorySize, ClassDirectories, ClassFiles, ClassNames, Classes, Methods,
};

use crate::{
    class_instance::{ClassInstance, Fields, PrimitiveArrayInstance},
    eval::{eval_method, EvalMethodValue, Frame, Locals, ValueException},
    gc::GcRef,
    initialize_class,
    jni::native_interface::NativeInterface,
    rv::{RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    GeneralError, State, ThreadData,
};

/// A struct that holds references to several of the important structures in their typical usage
/// This is repr-C because it needs to be able to be passed to native functions
#[repr(C)]
pub struct Env<'i> {
    // Interface MUST be the first field so that it is the first field in the jni
    pub interface: &'i NativeInterface,
    pub class_directories: ClassDirectories,
    pub class_names: ClassNames,
    pub class_files: ClassFiles,
    pub classes: Classes,
    pub packages: Packages,
    pub methods: Methods,
    pub(crate) state: State,
    pub(crate) tdata: ThreadData,
}

// TODO: A JavaString is obviously not exactly equivalent to a Rust string..
#[derive(Debug, Clone)]
pub struct JavaString(pub Vec<u16>);
impl MemorySize for JavaString {
    fn memory_size(&self) -> usize {
        self.0.memory_size()
    }
}

pub(crate) const fn signed_offset_16(lhs: u16, rhs: i16) -> Option<u16> {
    if rhs.is_negative() {
        if rhs == i16::MIN {
            None
        } else {
            lhs.checked_sub(rhs.abs() as u16)
        }
    } else {
        // It was not negative so it fits inside a u16
        #[allow(clippy::cast_sign_loss)]
        lhs.checked_add(rhs as u16)
    }
}

/// Construct a JVM String given some string
/// Note that `utf16_text` should be completely `RuntimeValuePrimitive::Char`
pub(crate) fn construct_string(
    env: &mut Env,
    utf16_text: Vec<RuntimeValuePrimitive>,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    // Create a char[] in utf16
    let char_arr_ref = {
        let char_arr_id = env.state.char_array_id(&mut env.class_names);
        let char_arr =
            PrimitiveArrayInstance::new(char_arr_id, RuntimeTypePrimitive::Char, utf16_text);
        env.state.gc.alloc(char_arr)
    };

    // Create the initial string reference, which could be uninitialized
    let string_ref = {
        // Initialize the string class if it somehow isn't already ready
        let string_id = env.state.string_class_id(&mut env.class_names);
        let string_ref = initialize_class(env, string_id)?;

        // Allocate the uninitialized instance
        let string_ref = match string_ref.into_value() {
            ValueException::Value(string_ref) => string_ref,
            ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
        };
        // TODO: Init fields better?
        env.state
            .gc
            .alloc(ClassInstance::new(string_id, string_ref, Fields::default()))
    };

    // Get the string constructor that would take a char[]
    let string_char_array_constructor = env.state.get_string_char_array_constructor(
        &env.class_directories,
        &mut env.class_names,
        &mut env.class_files,
        &mut env.methods,
    )?;
    // Evaluate the string constructor
    let string_inst = eval_method(
        env,
        string_char_array_constructor,
        Frame::new_locals(Locals::new_with_array([RuntimeValue::Reference(
            char_arr_ref.into_generic(),
        )])),
    )?;

    match string_inst {
        // We expect it to return nothing
        EvalMethodValue::ReturnVoid => (),
        // Returning something is weird..
        EvalMethodValue::Return(v) => {
            tracing::warn!("String constructor returned {:?}, ignoring..", v);
        }
        // If there was an exception, we simply pass it along
        EvalMethodValue::Exception(exc) => return Ok(ValueException::Exception(exc)),
    }

    Ok(ValueException::Value(string_ref))
}
