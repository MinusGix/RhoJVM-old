use std::{ffi::CStr, os::raw::c_char};

use rhojvm_base::code::method::MethodDescriptor;
use usize_cast::IntoIsize;

use crate::{
    class_instance::Instance,
    jni::{self, OpaqueClassMethod},
    method::NativeMethod,
    util::Env,
};

use super::{
    JBoolean, JByte, JChar, JClass, JFieldId, JInt, JObject, JSize, JThrowable, MethodNoArguments,
};

#[repr(C)]
pub struct NativeInterface {
    // These first four methods are reserved for future use by the JVM
    pub empty_0: MethodNoArguments,
    pub empty_1: MethodNoArguments,
    pub empty_2: MethodNoArguments,
    pub empty_3: MethodNoArguments,

    pub get_version: GetVersionFn,

    pub define_class: DefineClassFn,
    pub find_class: FindClassFn,

    pub from_reflected_method: MethodNoArguments,
    pub from_reflected_field: MethodNoArguments,
    pub to_reflected_method: MethodNoArguments,

    pub get_superclass: GetSuperclassFn,
    pub is_assignable_from: IsAssignableFromFn,

    pub to_reflected_field: MethodNoArguments,

    pub throw: ThrowFn,
    pub throw_new: ThrowNewFn,

    pub exception_occurred: ExceptionOccurredFn,
    pub exception_describe: ExceptionDescribeFn,
    pub exception_clear: ExceptionClearFn,
    pub fatal_error: FatalErrorFn,

    pub push_local_frame: MethodNoArguments,
    pub pop_local_frame: MethodNoArguments,

    pub new_global_ref: MethodNoArguments,
    pub delete_global_ref: MethodNoArguments,
    pub delete_local_ref: MethodNoArguments,
    pub is_same_object: MethodNoArguments,
    pub new_local_ref: MethodNoArguments,
    pub ensure_local_capacity: MethodNoArguments,

    pub alloc_object: MethodNoArguments,
    pub new_object: MethodNoArguments,
    pub new_object_v: MethodNoArguments,
    pub new_object_a: MethodNoArguments,

    pub get_object_class: MethodNoArguments,
    pub is_instance_of: MethodNoArguments,

    pub get_method_id: MethodNoArguments,

    pub call_object_method: MethodNoArguments,
    pub call_object_method_v: MethodNoArguments,
    pub call_object_method_a: MethodNoArguments,
    pub call_boolean_method: MethodNoArguments,
    pub call_boolean_method_v: MethodNoArguments,
    pub call_boolean_method_a: MethodNoArguments,
    pub call_byte_method: MethodNoArguments,
    pub call_byte_method_v: MethodNoArguments,
    pub call_byte_method_a: MethodNoArguments,
    pub call_char_method: MethodNoArguments,
    pub call_char_method_v: MethodNoArguments,
    pub call_char_method_a: MethodNoArguments,
    pub call_short_method: MethodNoArguments,
    pub call_short_method_v: MethodNoArguments,
    pub call_short_method_a: MethodNoArguments,
    pub call_int_method: MethodNoArguments,
    pub call_int_method_v: MethodNoArguments,
    pub call_int_method_a: MethodNoArguments,
    pub call_long_method: MethodNoArguments,
    pub call_long_method_v: MethodNoArguments,
    pub call_long_method_a: MethodNoArguments,
    pub call_float_method: MethodNoArguments,
    pub call_float_method_v: MethodNoArguments,
    pub call_float_method_a: MethodNoArguments,
    pub call_double_method: MethodNoArguments,
    pub call_double_method_v: MethodNoArguments,
    pub call_double_method_a: MethodNoArguments,
    pub call_void_method: MethodNoArguments,
    pub call_void_method_v: MethodNoArguments,
    pub call_void_method_a: MethodNoArguments,

    pub call_nonvirtual_object_method: MethodNoArguments,
    pub call_nonvirtual_object_method_v: MethodNoArguments,
    pub call_nonvirtual_object_method_a: MethodNoArguments,
    pub call_nonvirtual_boolean_method: MethodNoArguments,
    pub call_nonvirtual_boolean_method_v: MethodNoArguments,
    pub call_nonvirtual_boolean_method_a: MethodNoArguments,
    pub call_nonvirtual_byte_method: MethodNoArguments,
    pub call_nonvirtual_byte_method_v: MethodNoArguments,
    pub call_nonvirtual_byte_method_a: MethodNoArguments,
    pub call_nonvirtual_char_method: MethodNoArguments,
    pub call_nonvirtual_char_method_v: MethodNoArguments,
    pub call_nonvirtual_char_method_a: MethodNoArguments,
    pub call_nonvirtual_short_method: MethodNoArguments,
    pub call_nonvirtual_short_method_v: MethodNoArguments,
    pub call_nonvirtual_short_method_a: MethodNoArguments,
    pub call_nonvirtual_int_method: MethodNoArguments,
    pub call_nonvirtual_int_method_v: MethodNoArguments,
    pub call_nonvirtual_int_method_a: MethodNoArguments,
    pub call_nonvirtual_long_method: MethodNoArguments,
    pub call_nonvirtual_long_method_v: MethodNoArguments,
    pub call_nonvirtual_long_method_a: MethodNoArguments,
    pub call_nonvirtual_float_method: MethodNoArguments,
    pub call_nonvirtual_float_method_v: MethodNoArguments,
    pub call_nonvirtual_float_method_a: MethodNoArguments,
    pub call_nonvirtual_double_method: MethodNoArguments,
    pub call_nonvirtual_double_method_v: MethodNoArguments,
    pub call_nonvirtual_double_method_a: MethodNoArguments,
    pub call_nonvirtual_void_method: MethodNoArguments,
    pub call_nonvirtual_void_method_v: MethodNoArguments,
    pub call_nonvirtual_void_method_a: MethodNoArguments,

    pub get_field_id: GetFieldIdFn,

    pub get_object_field: MethodNoArguments,
    pub get_boolean_field: MethodNoArguments,
    pub get_byte_field: MethodNoArguments,
    pub get_char_field: MethodNoArguments,
    pub get_short_field: MethodNoArguments,
    pub get_int_field: MethodNoArguments,
    pub get_long_field: MethodNoArguments,
    pub get_float_field: MethodNoArguments,
    pub get_double_field: MethodNoArguments,
    pub set_object_field: MethodNoArguments,
    pub set_boolean_field: MethodNoArguments,
    pub set_byte_field: MethodNoArguments,
    pub set_char_field: MethodNoArguments,
    pub set_short_field: MethodNoArguments,
    pub set_int_field: MethodNoArguments,
    pub set_long_field: MethodNoArguments,
    pub set_float_field: MethodNoArguments,
    pub set_double_field: MethodNoArguments,

    pub get_static_method_id: MethodNoArguments,

    pub call_static_object_method: MethodNoArguments,
    pub call_static_object_method_v: MethodNoArguments,
    pub call_static_object_method_a: MethodNoArguments,
    pub call_static_boolean_method: MethodNoArguments,
    pub call_static_boolean_method_v: MethodNoArguments,
    pub call_static_boolean_method_a: MethodNoArguments,
    pub call_static_byte_method: MethodNoArguments,
    pub call_static_byte_method_v: MethodNoArguments,
    pub call_static_byte_method_a: MethodNoArguments,
    pub call_static_char_method: MethodNoArguments,
    pub call_static_char_method_v: MethodNoArguments,
    pub call_static_char_method_a: MethodNoArguments,
    pub call_static_short_method: MethodNoArguments,
    pub call_static_short_method_v: MethodNoArguments,
    pub call_static_short_method_a: MethodNoArguments,
    pub call_static_int_method: MethodNoArguments,
    pub call_static_int_method_v: MethodNoArguments,
    pub call_static_int_method_a: MethodNoArguments,
    pub call_static_long_method: MethodNoArguments,
    pub call_static_long_method_v: MethodNoArguments,
    pub call_static_long_method_a: MethodNoArguments,
    pub call_static_float_method: MethodNoArguments,
    pub call_static_float_method_v: MethodNoArguments,
    pub call_static_float_method_a: MethodNoArguments,
    pub call_static_double_method: MethodNoArguments,
    pub call_static_double_method_v: MethodNoArguments,
    pub call_static_double_method_a: MethodNoArguments,
    pub call_static_void_method: MethodNoArguments,
    pub call_static_void_method_v: MethodNoArguments,
    pub call_static_void_method_a: MethodNoArguments,

    pub get_static_field_id: MethodNoArguments,

    pub get_static_object_field: MethodNoArguments,
    pub get_static_boolean_field: MethodNoArguments,
    pub get_static_byte_field: MethodNoArguments,
    pub get_static_char_field: MethodNoArguments,
    pub get_static_short_field: MethodNoArguments,
    pub get_static_int_field: MethodNoArguments,
    pub get_static_long_field: MethodNoArguments,
    pub get_static_float_field: MethodNoArguments,
    pub get_static_double_field: MethodNoArguments,

    pub set_static_object_field: MethodNoArguments,
    pub set_static_boolean_field: MethodNoArguments,
    pub set_static_byte_field: MethodNoArguments,
    pub set_static_char_field: MethodNoArguments,
    pub set_static_short_field: MethodNoArguments,
    pub set_static_int_field: MethodNoArguments,
    pub set_static_long_field: MethodNoArguments,
    pub set_static_float_field: MethodNoArguments,
    pub set_static_double_field: MethodNoArguments,

    pub new_string: MethodNoArguments,

    pub get_string_length: MethodNoArguments,
    pub get_string_chars: MethodNoArguments,
    pub release_string_chars: MethodNoArguments,

    pub new_string_utf: MethodNoArguments,
    pub get_string_utf_length: MethodNoArguments,
    pub get_string_utf_chars: MethodNoArguments,
    pub release_string_utf_chars: MethodNoArguments,

    pub get_array_length: MethodNoArguments,

    pub new_object_array: MethodNoArguments,
    pub get_object_array_element: MethodNoArguments,
    pub set_object_array_element: MethodNoArguments,

    pub new_boolean_array: MethodNoArguments,
    pub new_byte_array: MethodNoArguments,
    pub new_char_array: MethodNoArguments,
    pub new_short_array: MethodNoArguments,
    pub new_int_array: MethodNoArguments,
    pub new_long_array: MethodNoArguments,
    pub new_float_array: MethodNoArguments,
    pub new_double_array: MethodNoArguments,

    pub get_boolean_array_elements: MethodNoArguments,
    pub get_byte_array_elements: MethodNoArguments,
    pub get_char_array_elements: MethodNoArguments,
    pub get_short_array_elements: MethodNoArguments,
    pub get_int_array_elements: MethodNoArguments,
    pub get_long_array_elements: MethodNoArguments,
    pub get_float_array_elements: MethodNoArguments,
    pub get_double_array_elements: MethodNoArguments,

    pub release_boolean_array_elements: MethodNoArguments,
    pub release_byte_array_elements: MethodNoArguments,
    pub release_char_array_elements: MethodNoArguments,
    pub release_short_array_elements: MethodNoArguments,
    pub release_int_array_elements: MethodNoArguments,
    pub release_long_array_elements: MethodNoArguments,
    pub release_float_array_elements: MethodNoArguments,
    pub release_double_array_elements: MethodNoArguments,

    pub get_boolean_array_region: MethodNoArguments,
    pub get_byte_array_region: MethodNoArguments,
    pub get_char_array_region: MethodNoArguments,
    pub get_short_array_region: MethodNoArguments,
    pub get_int_array_region: MethodNoArguments,
    pub get_long_array_region: MethodNoArguments,
    pub get_float_array_region: MethodNoArguments,
    pub get_double_array_region: MethodNoArguments,
    pub set_boolean_array_region: MethodNoArguments,
    pub set_byte_array_region: MethodNoArguments,
    pub set_char_array_region: MethodNoArguments,
    pub set_short_array_region: MethodNoArguments,
    pub set_int_array_region: MethodNoArguments,
    pub set_long_array_region: MethodNoArguments,
    pub set_float_array_region: MethodNoArguments,
    pub set_double_array_region: MethodNoArguments,

    pub register_natives: RegisterNativesFn,
    pub unregister_natives: MethodNoArguments,

    pub monitor_enter: MethodNoArguments,
    pub monitor_exit: MethodNoArguments,

    pub get_java_vm: MethodNoArguments,

    pub get_string_region: MethodNoArguments,
    pub get_string_utf_region: MethodNoArguments,

    pub get_primitive_array_critical: MethodNoArguments,
    pub release_primitive_array_critical: MethodNoArguments,

    pub get_string_critical: MethodNoArguments,
    pub release_string_critical: MethodNoArguments,

    pub new_weak_global_ref: MethodNoArguments,
    pub delete_weak_global_ref: MethodNoArguments,

    pub exception_check: ExceptionCheckFn,

    pub new_direct_byte_buffer: MethodNoArguments,
    pub get_direct_buffer_address: MethodNoArguments,
    pub get_direct_buffer_capacity: MethodNoArguments,

    pub get_object_ref_type: MethodNoArguments,

    pub get_module: GetModuleFn,
}
impl NativeInterface {
    pub(crate) fn new_typical() -> NativeInterface {
        NativeInterface {
            empty_0: unimpl_none,
            empty_1: unimpl_none,
            empty_2: unimpl_none,
            empty_3: unimpl_none,
            get_version,
            define_class,
            find_class,
            from_reflected_method: unimpl_none,
            from_reflected_field: unimpl_none,
            to_reflected_method: unimpl_none,
            get_superclass,
            is_assignable_from,
            to_reflected_field: unimpl_none,
            throw,
            throw_new,
            exception_occurred,
            exception_describe,
            exception_clear,
            fatal_error,
            push_local_frame: unimpl_none,
            pop_local_frame: unimpl_none,
            new_global_ref: unimpl_none,
            delete_global_ref: unimpl_none,
            delete_local_ref: unimpl_none,
            is_same_object: unimpl_none,
            new_local_ref: unimpl_none,
            ensure_local_capacity: unimpl_none,
            alloc_object: unimpl_none,
            new_object: unimpl_none,
            new_object_v: unimpl_none,
            new_object_a: unimpl_none,
            get_object_class: unimpl_none,
            is_instance_of: unimpl_none,
            get_method_id: unimpl_none,
            call_object_method: unimpl_none,
            call_object_method_v: unimpl_none,
            call_object_method_a: unimpl_none,
            call_boolean_method: unimpl_none,
            call_boolean_method_v: unimpl_none,
            call_boolean_method_a: unimpl_none,
            call_byte_method: unimpl_none,
            call_byte_method_v: unimpl_none,
            call_byte_method_a: unimpl_none,
            call_char_method: unimpl_none,
            call_char_method_v: unimpl_none,
            call_char_method_a: unimpl_none,
            call_short_method: unimpl_none,
            call_short_method_v: unimpl_none,
            call_short_method_a: unimpl_none,
            call_int_method: unimpl_none,
            call_int_method_v: unimpl_none,
            call_int_method_a: unimpl_none,
            call_long_method: unimpl_none,
            call_long_method_v: unimpl_none,
            call_long_method_a: unimpl_none,
            call_float_method: unimpl_none,
            call_float_method_v: unimpl_none,
            call_float_method_a: unimpl_none,
            call_double_method: unimpl_none,
            call_double_method_v: unimpl_none,
            call_double_method_a: unimpl_none,
            call_void_method: unimpl_none,
            call_void_method_v: unimpl_none,
            call_void_method_a: unimpl_none,
            call_nonvirtual_object_method: unimpl_none,
            call_nonvirtual_object_method_v: unimpl_none,
            call_nonvirtual_object_method_a: unimpl_none,
            call_nonvirtual_boolean_method: unimpl_none,
            call_nonvirtual_boolean_method_v: unimpl_none,
            call_nonvirtual_boolean_method_a: unimpl_none,
            call_nonvirtual_byte_method: unimpl_none,
            call_nonvirtual_byte_method_v: unimpl_none,
            call_nonvirtual_byte_method_a: unimpl_none,
            call_nonvirtual_char_method: unimpl_none,
            call_nonvirtual_char_method_v: unimpl_none,
            call_nonvirtual_char_method_a: unimpl_none,
            call_nonvirtual_short_method: unimpl_none,
            call_nonvirtual_short_method_v: unimpl_none,
            call_nonvirtual_short_method_a: unimpl_none,
            call_nonvirtual_int_method: unimpl_none,
            call_nonvirtual_int_method_v: unimpl_none,
            call_nonvirtual_int_method_a: unimpl_none,
            call_nonvirtual_long_method: unimpl_none,
            call_nonvirtual_long_method_v: unimpl_none,
            call_nonvirtual_long_method_a: unimpl_none,
            call_nonvirtual_float_method: unimpl_none,
            call_nonvirtual_float_method_v: unimpl_none,
            call_nonvirtual_float_method_a: unimpl_none,
            call_nonvirtual_double_method: unimpl_none,
            call_nonvirtual_double_method_v: unimpl_none,
            call_nonvirtual_double_method_a: unimpl_none,
            call_nonvirtual_void_method: unimpl_none,
            call_nonvirtual_void_method_v: unimpl_none,
            call_nonvirtual_void_method_a: unimpl_none,
            get_field_id,
            get_object_field: unimpl_none,
            get_boolean_field: unimpl_none,
            get_byte_field: unimpl_none,
            get_char_field: unimpl_none,
            get_short_field: unimpl_none,
            get_int_field: unimpl_none,
            get_long_field: unimpl_none,
            get_float_field: unimpl_none,
            get_double_field: unimpl_none,
            set_object_field: unimpl_none,
            set_boolean_field: unimpl_none,
            set_byte_field: unimpl_none,
            set_char_field: unimpl_none,
            set_short_field: unimpl_none,
            set_int_field: unimpl_none,
            set_long_field: unimpl_none,
            set_float_field: unimpl_none,
            set_double_field: unimpl_none,
            get_static_method_id: unimpl_none,
            call_static_object_method: unimpl_none,
            call_static_object_method_v: unimpl_none,
            call_static_object_method_a: unimpl_none,
            call_static_boolean_method: unimpl_none,
            call_static_boolean_method_v: unimpl_none,
            call_static_boolean_method_a: unimpl_none,
            call_static_byte_method: unimpl_none,
            call_static_byte_method_v: unimpl_none,
            call_static_byte_method_a: unimpl_none,
            call_static_char_method: unimpl_none,
            call_static_char_method_v: unimpl_none,
            call_static_char_method_a: unimpl_none,
            call_static_short_method: unimpl_none,
            call_static_short_method_v: unimpl_none,
            call_static_short_method_a: unimpl_none,
            call_static_int_method: unimpl_none,
            call_static_int_method_v: unimpl_none,
            call_static_int_method_a: unimpl_none,
            call_static_long_method: unimpl_none,
            call_static_long_method_v: unimpl_none,
            call_static_long_method_a: unimpl_none,
            call_static_float_method: unimpl_none,
            call_static_float_method_v: unimpl_none,
            call_static_float_method_a: unimpl_none,
            call_static_double_method: unimpl_none,
            call_static_double_method_v: unimpl_none,
            call_static_double_method_a: unimpl_none,
            call_static_void_method: unimpl_none,
            call_static_void_method_v: unimpl_none,
            call_static_void_method_a: unimpl_none,
            get_static_field_id: unimpl_none,
            get_static_object_field: unimpl_none,
            get_static_boolean_field: unimpl_none,
            get_static_byte_field: unimpl_none,
            get_static_char_field: unimpl_none,
            get_static_short_field: unimpl_none,
            get_static_int_field: unimpl_none,
            get_static_long_field: unimpl_none,
            get_static_float_field: unimpl_none,
            get_static_double_field: unimpl_none,
            set_static_object_field: unimpl_none,
            set_static_boolean_field: unimpl_none,
            set_static_byte_field: unimpl_none,
            set_static_char_field: unimpl_none,
            set_static_short_field: unimpl_none,
            set_static_int_field: unimpl_none,
            set_static_long_field: unimpl_none,
            set_static_float_field: unimpl_none,
            set_static_double_field: unimpl_none,
            new_string: unimpl_none,
            get_string_length: unimpl_none,
            get_string_chars: unimpl_none,
            release_string_chars: unimpl_none,
            new_string_utf: unimpl_none,
            get_string_utf_length: unimpl_none,
            get_string_utf_chars: unimpl_none,
            release_string_utf_chars: unimpl_none,
            get_array_length: unimpl_none,
            new_object_array: unimpl_none,
            get_object_array_element: unimpl_none,
            set_object_array_element: unimpl_none,
            new_boolean_array: unimpl_none,
            new_byte_array: unimpl_none,
            new_char_array: unimpl_none,
            new_short_array: unimpl_none,
            new_int_array: unimpl_none,
            new_long_array: unimpl_none,
            new_float_array: unimpl_none,
            new_double_array: unimpl_none,
            get_boolean_array_elements: unimpl_none,
            get_byte_array_elements: unimpl_none,
            get_char_array_elements: unimpl_none,
            get_short_array_elements: unimpl_none,
            get_int_array_elements: unimpl_none,
            get_long_array_elements: unimpl_none,
            get_float_array_elements: unimpl_none,
            get_double_array_elements: unimpl_none,
            release_boolean_array_elements: unimpl_none,
            release_byte_array_elements: unimpl_none,
            release_char_array_elements: unimpl_none,
            release_short_array_elements: unimpl_none,
            release_int_array_elements: unimpl_none,
            release_long_array_elements: unimpl_none,
            release_float_array_elements: unimpl_none,
            release_double_array_elements: unimpl_none,
            get_boolean_array_region: unimpl_none,
            get_byte_array_region: unimpl_none,
            get_char_array_region: unimpl_none,
            get_short_array_region: unimpl_none,
            get_int_array_region: unimpl_none,
            get_long_array_region: unimpl_none,
            get_float_array_region: unimpl_none,
            get_double_array_region: unimpl_none,
            set_boolean_array_region: unimpl_none,
            set_byte_array_region: unimpl_none,
            set_char_array_region: unimpl_none,
            set_short_array_region: unimpl_none,
            set_int_array_region: unimpl_none,
            set_long_array_region: unimpl_none,
            set_float_array_region: unimpl_none,
            set_double_array_region: unimpl_none,
            register_natives,
            unregister_natives: unimpl_none,
            monitor_enter: unimpl_none,
            monitor_exit: unimpl_none,
            get_java_vm: unimpl_none,
            get_string_region: unimpl_none,
            get_string_utf_region: unimpl_none,
            get_primitive_array_critical: unimpl_none,
            release_primitive_array_critical: unimpl_none,
            get_string_critical: unimpl_none,
            release_string_critical: unimpl_none,
            new_weak_global_ref: unimpl_none,
            delete_weak_global_ref: unimpl_none,
            exception_check,
            new_direct_byte_buffer: unimpl_none,
            get_direct_buffer_address: unimpl_none,
            get_direct_buffer_capacity: unimpl_none,
            get_object_ref_type: unimpl_none,
            get_module,
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
extern "C" fn unimpl_none(_: *mut Env) {
    unimpl("unimpl_none")
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

pub const REGISTER_NATIVE_SUCCESS: JInt = 0;
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
    assert!(!env.is_null(), "Native method's env was a nullptr");
    // Technically num_methods can't be 0, but lenient
    if num_methods == 0 {
        tracing::warn!("Native method passed in zero num_methods to RegisterNative");
        // We are lenient in this check
        if methods.is_null() {
            tracing::warn!("Native method passed in nullptr to RegisterNative");
        }
        return REGISTER_NATIVE_SUCCESS;
    }

    assert!(!methods.is_null(), "RegisterNative's methods was a nullptr");
    assert!(!class.is_null(), "RegisterNative's class was nullptr");

    // Safety: No other thread should be modifying this
    // We also assume it is a valid ref and hasn't been internally modified
    // in such a way as to make it invalid
    // We asserted that it is not null
    let class = *class;

    // Safety: No other thread should be using this
    // Though this relies on the native code being valid.
    // We already assert that it is not null
    let env = &mut *env;

    // The class id of the class we were given
    let class_id = match env.state.gc.deref(class) {
        Some(class_id) => match class_id {
            Instance::StaticClass(x) => x.id,
            Instance::Reference(x) => {
                // TODO: is is this actually fine? Maybe you're allowed to pass a reference to a
                // Class<T> class?
                tracing::warn!("Native method gave non static class reference to RegisterNatives");
                x.instanceof()
            }
        },
        None => todo!(),
    };

    for i in 0..num_methods {
        let method = methods.offset(i.into_isize());
        let name = (*method).name;
        let signature = (*method).signature;
        let fn_ptr = (*method).fn_ptr;

        // None of these should be null
        assert!(
            !name.is_null(),
            "RegisterNatives method's name was a nullptr"
        );
        assert!(
            !signature.is_null(),
            "RegisterNative's method's signature was a nullptr"
        );
        assert!(
            !fn_ptr.is_null(),
            "RegisterNative's method's function-ptr was a nullptr"
        );

        // Safety of both of these:
        // We've already checked that it is non-null
        // We know that we are not calling back into C-code, so as long as there is no
        // weird asynchronous shenanigans, it won't be freed out from under us.
        // As well, we are not modifying it from behind it, the shadowing making that somewhat
        // easier to rely on.
        // However, we have no guarantee that these actually end in a null-byte.
        // And, we have no guarantee that their length is < isize::MAX
        // TODO: Checked CStr constructor so we can provide exceptions on bad input?
        let name = CStr::from_ptr(name);
        let signature = CStr::from_ptr(signature);

        let descriptor =
            match MethodDescriptor::from_text(signature.to_bytes(), &mut env.class_names) {
                Ok(descriptor) => descriptor,
                Err(_) => todo!("Handle MethodDescriptor parse error"),
            };

        let method_id = match env.methods.load_method_from_desc(
            &env.class_directories,
            &mut env.class_names,
            &mut env.class_files,
            class_id,
            // Note that we're relying on the name ptr not aliasing env!
            name.to_bytes(),
            &descriptor,
        ) {
            Ok(method_id) => method_id,
            Err(_err) => {
                // todo!("Handle failing to find method");
                // TODO: Print name
                tracing::warn!("RegisterNatives: Failed to find method");
                // Indicates failure
                return -1;
            }
        };

        // Safety: we've already asserted that the function pointer is non-null
        // Otherwise, we're relying on the correctness of our caller
        let method_func =
            std::mem::transmute::<*mut std::ffi::c_void, jni::MethodClassNoArguments>(fn_ptr);

        let method_func = OpaqueClassMethod::new(method_func);

        // Store the native method on the method info structure so that it can be called when the
        // function is invoked
        env.state.method_info.modify_init_with(method_id, |data| {
            data.native_func = Some(NativeMethod::OpaqueRegistered(method_func));
        });
    }

    REGISTER_NATIVE_SUCCESS
}

pub type GetFieldIdFn = unsafe extern "C" fn(
    env: *mut Env,
    class: JClass,
    name: *const c_char,
    signature: *const c_char,
) -> JFieldId;
unsafe extern "C" fn get_field_id(
    env: *mut Env,
    class: JClass,
    name: *const c_char,
    signature: *const c_char,
) -> JFieldId {
    todo!("get_field_id");
}
