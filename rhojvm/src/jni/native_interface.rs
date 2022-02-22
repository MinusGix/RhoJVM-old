use std::ffi::CStr;

use usize_cast::IntoIsize;

use crate::util::Env;

use super::{JBoolean, JByte, JChar, JClass, JInt, JObject, JSize, JThrowable};

#[repr(C)]
pub struct NativeInterface {
    // These first four methods are reserved for future use by the JVM
    pub empty_0: NullMethod,
    pub empty_1: NullMethod,
    pub empty_2: NullMethod,
    pub empty_3: NullMethod,

    pub get_version: GetVersionFn,

    pub define_class: DefineClassFn,
    pub find_class: FindClassFn,

    pub from_reflected_method: NullMethod,
    pub from_reflected_field: NullMethod,
    pub to_reflected_method: NullMethod,

    pub get_superclass: GetSuperclassFn,
    pub is_assignable_from: IsAssignableFromFn,

    pub to_reflected_field: NullMethod,

    pub throw: ThrowFn,
    pub throw_new: ThrowNewFn,

    pub exception_occurred: ExceptionOccurredFn,
    pub exception_describe: ExceptionDescribeFn,
    pub exception_clear: ExceptionClearFn,
    pub fatal_error: FatalErrorFn,

    pub push_local_frame: NullMethod,
    pub pop_local_frame: NullMethod,

    pub new_global_ref: NullMethod,
    pub delete_global_ref: NullMethod,
    pub delete_local_ref: NullMethod,
    pub is_same_object: NullMethod,
    pub new_local_ref: NullMethod,
    pub ensure_local_capacity: NullMethod,

    pub alloc_object: NullMethod,
    pub new_object: NullMethod,
    pub new_object_v: NullMethod,
    pub new_object_a: NullMethod,

    pub get_object_class: NullMethod,
    pub is_instance_of: NullMethod,

    pub get_method_id: NullMethod,

    pub call_object_method: NullMethod,
    pub call_object_method_v: NullMethod,
    pub call_object_method_a: NullMethod,
    pub call_boolean_method: NullMethod,
    pub call_boolean_method_v: NullMethod,
    pub call_boolean_method_a: NullMethod,
    pub call_byte_method: NullMethod,
    pub call_byte_method_v: NullMethod,
    pub call_byte_method_a: NullMethod,
    pub call_char_method: NullMethod,
    pub call_char_method_v: NullMethod,
    pub call_char_method_a: NullMethod,
    pub call_short_method: NullMethod,
    pub call_short_method_v: NullMethod,
    pub call_short_method_a: NullMethod,
    pub call_int_method: NullMethod,
    pub call_int_method_v: NullMethod,
    pub call_int_method_a: NullMethod,
    pub call_long_method: NullMethod,
    pub call_long_method_v: NullMethod,
    pub call_long_method_a: NullMethod,
    pub call_float_method: NullMethod,
    pub call_float_method_v: NullMethod,
    pub call_float_method_a: NullMethod,
    pub call_double_method: NullMethod,
    pub call_double_method_v: NullMethod,
    pub call_double_method_a: NullMethod,
    pub call_void_method: NullMethod,
    pub call_void_method_v: NullMethod,
    pub call_void_method_a: NullMethod,

    pub call_nonvirtual_object_method: NullMethod,
    pub call_nonvirtual_object_method_v: NullMethod,
    pub call_nonvirtual_object_method_a: NullMethod,
    pub call_nonvirtual_boolean_method: NullMethod,
    pub call_nonvirtual_boolean_method_v: NullMethod,
    pub call_nonvirtual_boolean_method_a: NullMethod,
    pub call_nonvirtual_byte_method: NullMethod,
    pub call_nonvirtual_byte_method_v: NullMethod,
    pub call_nonvirtual_byte_method_a: NullMethod,
    pub call_nonvirtual_char_method: NullMethod,
    pub call_nonvirtual_char_method_v: NullMethod,
    pub call_nonvirtual_char_method_a: NullMethod,
    pub call_nonvirtual_short_method: NullMethod,
    pub call_nonvirtual_short_method_v: NullMethod,
    pub call_nonvirtual_short_method_a: NullMethod,
    pub call_nonvirtual_int_method: NullMethod,
    pub call_nonvirtual_int_method_v: NullMethod,
    pub call_nonvirtual_int_method_a: NullMethod,
    pub call_nonvirtual_long_method: NullMethod,
    pub call_nonvirtual_long_method_v: NullMethod,
    pub call_nonvirtual_long_method_a: NullMethod,
    pub call_nonvirtual_float_method: NullMethod,
    pub call_nonvirtual_float_method_v: NullMethod,
    pub call_nonvirtual_float_method_a: NullMethod,
    pub call_nonvirtual_double_method: NullMethod,
    pub call_nonvirtual_double_method_v: NullMethod,
    pub call_nonvirtual_double_method_a: NullMethod,
    pub call_nonvirtual_void_method: NullMethod,
    pub call_nonvirtual_void_method_v: NullMethod,
    pub call_nonvirtual_void_method_a: NullMethod,

    pub get_field_id: NullMethod,

    pub get_object_field: NullMethod,
    pub get_boolean_field: NullMethod,
    pub get_byte_field: NullMethod,
    pub get_char_field: NullMethod,
    pub get_short_field: NullMethod,
    pub get_int_field: NullMethod,
    pub get_long_field: NullMethod,
    pub get_float_field: NullMethod,
    pub get_double_field: NullMethod,
    pub set_object_field: NullMethod,
    pub set_boolean_field: NullMethod,
    pub set_byte_field: NullMethod,
    pub set_char_field: NullMethod,
    pub set_short_field: NullMethod,
    pub set_int_field: NullMethod,
    pub set_long_field: NullMethod,
    pub set_float_field: NullMethod,
    pub set_double_field: NullMethod,

    pub get_static_method_id: NullMethod,

    pub call_static_object_method: NullMethod,
    pub call_static_object_method_v: NullMethod,
    pub call_static_object_method_a: NullMethod,
    pub call_static_boolean_method: NullMethod,
    pub call_static_boolean_method_v: NullMethod,
    pub call_static_boolean_method_a: NullMethod,
    pub call_static_byte_method: NullMethod,
    pub call_static_byte_method_v: NullMethod,
    pub call_static_byte_method_a: NullMethod,
    pub call_static_char_method: NullMethod,
    pub call_static_char_method_v: NullMethod,
    pub call_static_char_method_a: NullMethod,
    pub call_static_short_method: NullMethod,
    pub call_static_short_method_v: NullMethod,
    pub call_static_short_method_a: NullMethod,
    pub call_static_int_method: NullMethod,
    pub call_static_int_method_v: NullMethod,
    pub call_static_int_method_a: NullMethod,
    pub call_static_long_method: NullMethod,
    pub call_static_long_method_v: NullMethod,
    pub call_static_long_method_a: NullMethod,
    pub call_static_float_method: NullMethod,
    pub call_static_float_method_v: NullMethod,
    pub call_static_float_method_a: NullMethod,
    pub call_static_double_method: NullMethod,
    pub call_static_double_method_v: NullMethod,
    pub call_static_double_method_a: NullMethod,
    pub call_static_void_method: NullMethod,
    pub call_static_void_method_v: NullMethod,
    pub call_static_void_method_a: NullMethod,

    pub get_static_field_id: NullMethod,

    pub get_static_object_field: NullMethod,
    pub get_static_boolean_field: NullMethod,
    pub get_static_byte_field: NullMethod,
    pub get_static_char_field: NullMethod,
    pub get_static_short_field: NullMethod,
    pub get_static_int_field: NullMethod,
    pub get_static_long_field: NullMethod,
    pub get_static_float_field: NullMethod,
    pub get_static_double_field: NullMethod,

    pub set_static_object_field: NullMethod,
    pub set_static_boolean_field: NullMethod,
    pub set_static_byte_field: NullMethod,
    pub set_static_char_field: NullMethod,
    pub set_static_short_field: NullMethod,
    pub set_static_int_field: NullMethod,
    pub set_static_long_field: NullMethod,
    pub set_static_float_field: NullMethod,
    pub set_static_double_field: NullMethod,

    pub new_string: NullMethod,

    pub get_string_length: NullMethod,
    pub get_string_chars: NullMethod,
    pub release_string_chars: NullMethod,

    pub new_string_utf: NullMethod,
    pub get_string_utf_length: NullMethod,
    pub get_string_utf_chars: NullMethod,
    pub release_string_utf_chars: NullMethod,

    pub get_array_length: NullMethod,

    pub new_object_array: NullMethod,
    pub get_object_array_element: NullMethod,
    pub set_object_array_element: NullMethod,

    pub new_boolean_array: NullMethod,
    pub new_byte_array: NullMethod,
    pub new_char_array: NullMethod,
    pub new_short_array: NullMethod,
    pub new_int_array: NullMethod,
    pub new_long_array: NullMethod,
    pub new_float_array: NullMethod,
    pub new_double_array: NullMethod,

    pub get_boolean_array_elements: NullMethod,
    pub get_byte_array_elements: NullMethod,
    pub get_char_array_elements: NullMethod,
    pub get_short_array_elements: NullMethod,
    pub get_int_array_elements: NullMethod,
    pub get_long_array_elements: NullMethod,
    pub get_float_array_elements: NullMethod,
    pub get_double_array_elements: NullMethod,

    pub release_boolean_array_elements: NullMethod,
    pub release_byte_array_elements: NullMethod,
    pub release_char_array_elements: NullMethod,
    pub release_short_array_elements: NullMethod,
    pub release_int_array_elements: NullMethod,
    pub release_long_array_elements: NullMethod,
    pub release_float_array_elements: NullMethod,
    pub release_double_array_elements: NullMethod,

    pub get_boolean_array_region: NullMethod,
    pub get_byte_array_region: NullMethod,
    pub get_char_array_region: NullMethod,
    pub get_short_array_region: NullMethod,
    pub get_int_array_region: NullMethod,
    pub get_long_array_region: NullMethod,
    pub get_float_array_region: NullMethod,
    pub get_double_array_region: NullMethod,
    pub set_boolean_array_region: NullMethod,
    pub set_byte_array_region: NullMethod,
    pub set_char_array_region: NullMethod,
    pub set_short_array_region: NullMethod,
    pub set_int_array_region: NullMethod,
    pub set_long_array_region: NullMethod,
    pub set_float_array_region: NullMethod,
    pub set_double_array_region: NullMethod,

    pub register_natives: RegisterNativesFn,
    pub unregister_natives: NullMethod,

    pub monitor_enter: NullMethod,
    pub monitor_exit: NullMethod,

    pub get_java_vm: NullMethod,

    pub get_string_region: NullMethod,
    pub get_string_utf_region: NullMethod,

    pub get_primitive_array_critical: NullMethod,
    pub release_primitive_array_critical: NullMethod,

    pub get_string_critical: NullMethod,
    pub release_string_critical: NullMethod,

    pub new_weak_global_ref: NullMethod,
    pub delete_weak_global_ref: NullMethod,

    pub exception_check: ExceptionCheckFn,

    pub new_direct_byte_buffer: NullMethod,
    pub get_direct_buffer_address: NullMethod,
    pub get_direct_buffer_capacity: NullMethod,

    pub get_object_ref_type: NullMethod,

    pub get_module: NullMethod,
}
impl NativeInterface {
    pub(crate) fn new_typical() -> NativeInterface {
        NativeInterface {
            empty_0: NullMethod::default(),
            empty_1: NullMethod::default(),
            empty_2: NullMethod::default(),
            empty_3: NullMethod::default(),
            get_version,
            define_class,
            find_class,
            from_reflected_method: NullMethod::default(),
            from_reflected_field: NullMethod::default(),
            to_reflected_method: NullMethod::default(),
            get_superclass,
            is_assignable_from,
            to_reflected_field: NullMethod::default(),
            throw,
            throw_new,
            exception_occurred,
            exception_describe,
            exception_clear,
            fatal_error,
            push_local_frame: NullMethod::default(),
            pop_local_frame: NullMethod::default(),
            new_global_ref: NullMethod::default(),
            delete_global_ref: NullMethod::default(),
            delete_local_ref: NullMethod::default(),
            is_same_object: NullMethod::default(),
            new_local_ref: NullMethod::default(),
            ensure_local_capacity: NullMethod::default(),
            alloc_object: NullMethod::default(),
            new_object: NullMethod::default(),
            new_object_v: NullMethod::default(),
            new_object_a: NullMethod::default(),
            get_object_class: NullMethod::default(),
            is_instance_of: NullMethod::default(),
            get_method_id: NullMethod::default(),
            call_object_method: NullMethod::default(),
            call_object_method_v: NullMethod::default(),
            call_object_method_a: NullMethod::default(),
            call_boolean_method: NullMethod::default(),
            call_boolean_method_v: NullMethod::default(),
            call_boolean_method_a: NullMethod::default(),
            call_byte_method: NullMethod::default(),
            call_byte_method_v: NullMethod::default(),
            call_byte_method_a: NullMethod::default(),
            call_char_method: NullMethod::default(),
            call_char_method_v: NullMethod::default(),
            call_char_method_a: NullMethod::default(),
            call_short_method: NullMethod::default(),
            call_short_method_v: NullMethod::default(),
            call_short_method_a: NullMethod::default(),
            call_int_method: NullMethod::default(),
            call_int_method_v: NullMethod::default(),
            call_int_method_a: NullMethod::default(),
            call_long_method: NullMethod::default(),
            call_long_method_v: NullMethod::default(),
            call_long_method_a: NullMethod::default(),
            call_float_method: NullMethod::default(),
            call_float_method_v: NullMethod::default(),
            call_float_method_a: NullMethod::default(),
            call_double_method: NullMethod::default(),
            call_double_method_v: NullMethod::default(),
            call_double_method_a: NullMethod::default(),
            call_void_method: NullMethod::default(),
            call_void_method_v: NullMethod::default(),
            call_void_method_a: NullMethod::default(),
            call_nonvirtual_object_method: NullMethod::default(),
            call_nonvirtual_object_method_v: NullMethod::default(),
            call_nonvirtual_object_method_a: NullMethod::default(),
            call_nonvirtual_boolean_method: NullMethod::default(),
            call_nonvirtual_boolean_method_v: NullMethod::default(),
            call_nonvirtual_boolean_method_a: NullMethod::default(),
            call_nonvirtual_byte_method: NullMethod::default(),
            call_nonvirtual_byte_method_v: NullMethod::default(),
            call_nonvirtual_byte_method_a: NullMethod::default(),
            call_nonvirtual_char_method: NullMethod::default(),
            call_nonvirtual_char_method_v: NullMethod::default(),
            call_nonvirtual_char_method_a: NullMethod::default(),
            call_nonvirtual_short_method: NullMethod::default(),
            call_nonvirtual_short_method_v: NullMethod::default(),
            call_nonvirtual_short_method_a: NullMethod::default(),
            call_nonvirtual_int_method: NullMethod::default(),
            call_nonvirtual_int_method_v: NullMethod::default(),
            call_nonvirtual_int_method_a: NullMethod::default(),
            call_nonvirtual_long_method: NullMethod::default(),
            call_nonvirtual_long_method_v: NullMethod::default(),
            call_nonvirtual_long_method_a: NullMethod::default(),
            call_nonvirtual_float_method: NullMethod::default(),
            call_nonvirtual_float_method_v: NullMethod::default(),
            call_nonvirtual_float_method_a: NullMethod::default(),
            call_nonvirtual_double_method: NullMethod::default(),
            call_nonvirtual_double_method_v: NullMethod::default(),
            call_nonvirtual_double_method_a: NullMethod::default(),
            call_nonvirtual_void_method: NullMethod::default(),
            call_nonvirtual_void_method_v: NullMethod::default(),
            call_nonvirtual_void_method_a: NullMethod::default(),
            get_field_id: NullMethod::default(),
            get_object_field: NullMethod::default(),
            get_boolean_field: NullMethod::default(),
            get_byte_field: NullMethod::default(),
            get_char_field: NullMethod::default(),
            get_short_field: NullMethod::default(),
            get_int_field: NullMethod::default(),
            get_long_field: NullMethod::default(),
            get_float_field: NullMethod::default(),
            get_double_field: NullMethod::default(),
            set_object_field: NullMethod::default(),
            set_boolean_field: NullMethod::default(),
            set_byte_field: NullMethod::default(),
            set_char_field: NullMethod::default(),
            set_short_field: NullMethod::default(),
            set_int_field: NullMethod::default(),
            set_long_field: NullMethod::default(),
            set_float_field: NullMethod::default(),
            set_double_field: NullMethod::default(),
            get_static_method_id: NullMethod::default(),
            call_static_object_method: NullMethod::default(),
            call_static_object_method_v: NullMethod::default(),
            call_static_object_method_a: NullMethod::default(),
            call_static_boolean_method: NullMethod::default(),
            call_static_boolean_method_v: NullMethod::default(),
            call_static_boolean_method_a: NullMethod::default(),
            call_static_byte_method: NullMethod::default(),
            call_static_byte_method_v: NullMethod::default(),
            call_static_byte_method_a: NullMethod::default(),
            call_static_char_method: NullMethod::default(),
            call_static_char_method_v: NullMethod::default(),
            call_static_char_method_a: NullMethod::default(),
            call_static_short_method: NullMethod::default(),
            call_static_short_method_v: NullMethod::default(),
            call_static_short_method_a: NullMethod::default(),
            call_static_int_method: NullMethod::default(),
            call_static_int_method_v: NullMethod::default(),
            call_static_int_method_a: NullMethod::default(),
            call_static_long_method: NullMethod::default(),
            call_static_long_method_v: NullMethod::default(),
            call_static_long_method_a: NullMethod::default(),
            call_static_float_method: NullMethod::default(),
            call_static_float_method_v: NullMethod::default(),
            call_static_float_method_a: NullMethod::default(),
            call_static_double_method: NullMethod::default(),
            call_static_double_method_v: NullMethod::default(),
            call_static_double_method_a: NullMethod::default(),
            call_static_void_method: NullMethod::default(),
            call_static_void_method_v: NullMethod::default(),
            call_static_void_method_a: NullMethod::default(),
            get_static_field_id: NullMethod::default(),
            get_static_object_field: NullMethod::default(),
            get_static_boolean_field: NullMethod::default(),
            get_static_byte_field: NullMethod::default(),
            get_static_char_field: NullMethod::default(),
            get_static_short_field: NullMethod::default(),
            get_static_int_field: NullMethod::default(),
            get_static_long_field: NullMethod::default(),
            get_static_float_field: NullMethod::default(),
            get_static_double_field: NullMethod::default(),
            set_static_object_field: NullMethod::default(),
            set_static_boolean_field: NullMethod::default(),
            set_static_byte_field: NullMethod::default(),
            set_static_char_field: NullMethod::default(),
            set_static_short_field: NullMethod::default(),
            set_static_int_field: NullMethod::default(),
            set_static_long_field: NullMethod::default(),
            set_static_float_field: NullMethod::default(),
            set_static_double_field: NullMethod::default(),
            new_string: NullMethod::default(),
            get_string_length: NullMethod::default(),
            get_string_chars: NullMethod::default(),
            release_string_chars: NullMethod::default(),
            new_string_utf: NullMethod::default(),
            get_string_utf_length: NullMethod::default(),
            get_string_utf_chars: NullMethod::default(),
            release_string_utf_chars: NullMethod::default(),
            get_array_length: NullMethod::default(),
            new_object_array: NullMethod::default(),
            get_object_array_element: NullMethod::default(),
            set_object_array_element: NullMethod::default(),
            new_boolean_array: NullMethod::default(),
            new_byte_array: NullMethod::default(),
            new_char_array: NullMethod::default(),
            new_short_array: NullMethod::default(),
            new_int_array: NullMethod::default(),
            new_long_array: NullMethod::default(),
            new_float_array: NullMethod::default(),
            new_double_array: NullMethod::default(),
            get_boolean_array_elements: NullMethod::default(),
            get_byte_array_elements: NullMethod::default(),
            get_char_array_elements: NullMethod::default(),
            get_short_array_elements: NullMethod::default(),
            get_int_array_elements: NullMethod::default(),
            get_long_array_elements: NullMethod::default(),
            get_float_array_elements: NullMethod::default(),
            get_double_array_elements: NullMethod::default(),
            release_boolean_array_elements: NullMethod::default(),
            release_byte_array_elements: NullMethod::default(),
            release_char_array_elements: NullMethod::default(),
            release_short_array_elements: NullMethod::default(),
            release_int_array_elements: NullMethod::default(),
            release_long_array_elements: NullMethod::default(),
            release_float_array_elements: NullMethod::default(),
            release_double_array_elements: NullMethod::default(),
            get_boolean_array_region: NullMethod::default(),
            get_byte_array_region: NullMethod::default(),
            get_char_array_region: NullMethod::default(),
            get_short_array_region: NullMethod::default(),
            get_int_array_region: NullMethod::default(),
            get_long_array_region: NullMethod::default(),
            get_float_array_region: NullMethod::default(),
            get_double_array_region: NullMethod::default(),
            set_boolean_array_region: NullMethod::default(),
            set_byte_array_region: NullMethod::default(),
            set_char_array_region: NullMethod::default(),
            set_short_array_region: NullMethod::default(),
            set_int_array_region: NullMethod::default(),
            set_long_array_region: NullMethod::default(),
            set_float_array_region: NullMethod::default(),
            set_double_array_region: NullMethod::default(),
            register_natives,
            unregister_natives: NullMethod::default(),
            monitor_enter: NullMethod::default(),
            monitor_exit: NullMethod::default(),
            get_java_vm: NullMethod::default(),
            get_string_region: NullMethod::default(),
            get_string_utf_region: NullMethod::default(),
            get_primitive_array_critical: NullMethod::default(),
            release_primitive_array_critical: NullMethod::default(),
            get_string_critical: NullMethod::default(),
            release_string_critical: NullMethod::default(),
            new_weak_global_ref: NullMethod::default(),
            delete_weak_global_ref: NullMethod::default(),
            exception_check,
            new_direct_byte_buffer: NullMethod::default(),
            get_direct_buffer_address: NullMethod::default(),
            get_direct_buffer_capacity: NullMethod::default(),
            get_object_ref_type: NullMethod::default(),
            get_module: NullMethod::default(),
        }
    }
}

/// A 'method' that is just a null pointer
#[repr(transparent)]
pub struct NullMethod(*const std::ffi::c_void);
impl Default for NullMethod {
    fn default() -> Self {
        Self(std::ptr::null())
    }
}

pub type UnimplNoneFn = unsafe extern "C" fn(env: *mut Env);
fn unimpl_none<T: std::any::Any>(_: *mut Env) {
    unimpl(std::any::type_name::<T>())
}

// We don't need to specify return type since it will abort
pub type UnimplOnePtrFn = unsafe extern "C" fn(env: *mut Env, ptr: *mut std::ffi::c_void);
fn unimpl_one_ptr<T: std::any::Any>(_: *mut Env, _: *mut std::ffi::c_void) {
    unimpl(std::any::type_name::<T>())
}

fn unimpl(message: &str) -> ! {
    use std::io::Write;
    {
        println!("Unimplemented Function: {}", message);
        let mut stdout = std::io::stdout();
        let _x = stdout.flush();
    };
    std::process::abort();
}

// We don't say these functions take a reference since we don't control the C code that will call it
// and they may pass a null pointer
// As well, the exact functions we use are, at least currently, private. So that we don't stabilize
// something before it is finished.
// Such as `get_version`, which in the future will probably be unsafe since it will need to access
// the env to get data

pub type GetVersionFn = unsafe extern "C" fn(env: *mut Env) -> JInt;
extern "C" fn get_version(_: *mut Env) -> JInt {
    // TODO: Return a better number depending on which JVM we're acting as
    // Currently this is the JDK8 number
    0x0001_0008
}

/// Loads a class from a buffer of raw class data.
/// The buffer containing the raw class data is not referenced by the VM after the `DefineClass`  
/// call returns and it may be discarded if desired.
/// `env` must not be null
/// `name`: name of the class/interface to be defined. Encoded in modified UTF-8. It may be null,
/// or it must match the name encoded within the class file data.
/// `loader`: A class loader assigned to the defined class. This may be null, which indicates the
/// bootstrap class loader.
/// `buf`: buffer container the .class file data. A null value will cause a `ClassFormatErfror`
/// `buf_len`: Length of the buffer
/// Returns: A java class object or null if an error occurs
/// # Throws
/// `ClassFormatErorr`: If the class data does not specify a valid class
/// `ClassCircularityError`: If a class/interface would be its own superclass/superinterface
/// `OutOfMemoryError`: If the system runs out of memory
/// `SecurityException`: If the caller attempts to define a class in the 'java' package tree
pub type DefineClassFn = unsafe extern "C" fn(
    env: *mut Env,
    name: *const JChar,
    loader: JObject,
    buf: *const JByte,
    buf_len: JSize,
) -> JClass;
extern "C" fn define_class(
    env: *mut Env,
    name: *const JChar,
    loader: JObject,
    buf: *const JByte,
    buf_len: JSize,
) -> JClass {
    unimpl("DefineClass")
}

pub type FindClassFn = unsafe extern "C" fn(env: *mut Env, name: *const JChar) -> JClass;
extern "C" fn find_class(env: *mut Env, name: *const JChar) -> JClass {
    unimpl("FindClass")
}

pub type GetSuperclassFn = unsafe extern "C" fn(env: *mut Env, class: JClass) -> JClass;
extern "C" fn get_superclass(env: *mut Env, class: JClass) -> JClass {
    unimpl("GetSuperclass")
}

pub type IsAssignableFromFn =
    unsafe extern "C" fn(env: *mut Env, class: JClass, target_class: JClass) -> JBoolean;
extern "C" fn is_assignable_from(env: *mut Env, class: JClass, target_class: JClass) -> JBoolean {
    unimpl("IsAssignableFrom")
}

pub type GetModuleFn = unsafe extern "C" fn(env: *mut Env, class: JClass) -> JObject;
extern "C" fn get_module(env: *mut Env, class: JClass) -> JObject {
    unimpl("GetModule")
}

pub type ThrowFn = unsafe extern "C" fn(env: *mut Env, obj: JThrowable) -> JInt;
extern "C" fn throw(env: *mut Env, obj: JThrowable) -> JInt {
    unimpl("Throw")
}

pub type ThrowNewFn =
    unsafe extern "C" fn(env: *mut Env, class: JClass, message: *const JChar) -> JInt;
extern "C" fn throw_new(env: *mut Env, class: JClass, message: *const JChar) -> JInt {
    unimpl("ThrowNew")
}

pub type ExceptionOccurredFn = unsafe extern "C" fn(env: *mut Env) -> JThrowable;
extern "C" fn exception_occurred(env: *mut Env) -> JThrowable {
    unimpl("ExceptionOccurred")
}

pub type ExceptionDescribeFn = unsafe extern "C" fn(env: *mut Env);
extern "C" fn exception_describe(env: *mut Env) {
    unimpl("ExceptionDescribe")
}

pub type ExceptionClearFn = unsafe extern "C" fn(env: *mut Env);
extern "C" fn exception_clear(env: *mut Env) {
    unimpl("ExceptionClear")
}

pub type FatalErrorFn = unsafe extern "C" fn(env: *mut Env, msg: *const JChar);
extern "C" fn fatal_error(env: *mut Env, msg: *const JChar) {
    unimpl("FatalError")
}

pub type ExceptionCheckFn = unsafe extern "C" fn(env: *mut Env) -> JBoolean;
extern "C" fn exception_check(env: *mut Env) -> JBoolean {
    unimpl("ExceptionCheck")
}

#[repr(C)]
pub struct JNINativeMethod {
    pub name: *mut std::os::raw::c_char,
    pub signature: *mut std::os::raw::c_char,
    pub fn_ptr: *mut std::os::raw::c_void,
}

/// Registers native methods with the class specified.
/// The methods parameter specifies an array of [`JNINativeMethod`]s that contain the names,
/// signatures, and function pointers of the native methods.
/// The name and signature fields of the [`JNINativeMethod`] are pointers to modified UTF-8 strings.
pub type RegisterNativesFn = unsafe extern "C" fn(
    env: *mut Env,
    class: JClass,
    methods: *const JNINativeMethod,
    num_methods: JInt,
) -> JInt;
unsafe extern "C" fn register_natives(
    env: *mut Env,
    class: JClass,
    methods: *const JNINativeMethod,
    num_methods: JInt,
) -> JInt {
    for i in 0..num_methods {
        let method = methods.offset(i.into_isize());
        let name = CStr::from_ptr((*method).name);
        let signature = CStr::from_ptr((*method).signature);
        let fn_ptr = (*method).fn_ptr;
        tracing::info!(
            "\tRegistering {} :: {} => 0x{:X}",
            name.to_str().unwrap(),
            signature.to_str().unwrap(),
            fn_ptr as usize,
        );
    }

    unimpl("RegisterNatives");

    return 0;
}
