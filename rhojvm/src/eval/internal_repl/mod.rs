//! Internal replacements for native functions  

use crate::{
    jni::{JInt, JObject, MethodClassNoArguments, OpaqueClassMethod},
    util::Env,
};

mod class;
pub mod field;
pub mod method_handle;
pub mod method_handle_info;
mod method_type;
mod object;
mod primitive;
pub mod reflect_array;
pub mod reflection;
pub mod runtime;
pub mod string;
mod system;
mod system_class_loader;
mod thread;
mod unsafe_;

/// A garbage value intended for use in returns that shouldn't be used, because an exception was
/// thrown
pub(crate) const JINT_GARBAGE: JInt = JInt::MAX;

// TODO: Should we use something like PHF? Every native lookup is going to check this array
// for if it exists, which does make them all more expensive for this case. PHF would probably be
// faster than whatever llvm optimizes this to.

/// Converts function ptr into opaque method ptr for use by native calling code
/// # Safety
unsafe fn into_opaque2ret<R>(
    f: unsafe extern "C" fn(*mut Env<'_>, JObject) -> R,
) -> OpaqueClassMethod {
    OpaqueClassMethod::new(std::mem::transmute::<
        unsafe extern "C" fn(*mut Env<'_>, JObject) -> R,
        MethodClassNoArguments,
    >(f))
}

/// Converts function ptr into opaque method ptr for use by native calling code
/// # Safety
unsafe fn into_opaque3ret<R, A>(
    f: unsafe extern "C" fn(*mut Env<'_>, JObject, A) -> R,
) -> OpaqueClassMethod {
    OpaqueClassMethod::new(std::mem::transmute::<
        unsafe extern "C" fn(*mut Env<'_>, JObject, A) -> R,
        MethodClassNoArguments,
    >(f))
}

/// Converts function ptr into opaque method ptr for use by native calling code
/// # Safety
unsafe fn into_opaque4ret<R, A, B>(
    f: unsafe extern "C" fn(*mut Env<'_>, JObject, A, B) -> R,
) -> OpaqueClassMethod {
    OpaqueClassMethod::new(std::mem::transmute::<
        unsafe extern "C" fn(*mut Env<'_>, JObject, A, B) -> R,
        MethodClassNoArguments,
    >(f))
}

/// Converts function ptr into opaque method ptr for use by native calling code
/// # Safety
unsafe fn into_opaque5ret<R, A, B, C>(
    f: unsafe extern "C" fn(*mut Env<'_>, JObject, A, B, C) -> R,
) -> OpaqueClassMethod {
    OpaqueClassMethod::new(std::mem::transmute::<
        unsafe extern "C" fn(*mut Env<'_>, JObject, A, B, C) -> R,
        MethodClassNoArguments,
    >(f))
}

/// Converts function ptr into opaque method ptr for use by native calling code
/// # Safety
unsafe fn into_opaque7ret<R, A, B, C, D, E>(
    f: unsafe extern "C" fn(*mut Env<'_>, JObject, A, B, C, D, E) -> R,
) -> OpaqueClassMethod {
    OpaqueClassMethod::new(std::mem::transmute::<
        unsafe extern "C" fn(*mut Env<'_>, JObject, A, B, C, D, E) -> R,
        MethodClassNoArguments,
    >(f))
}

pub(crate) fn find_internal_rho_native_method(name: &[u8]) -> Option<OpaqueClassMethod> {
    // Remove any ending null byte if there is one, since that makes our matching easier.
    let name = if let Some(name) = name.strip_suffix(b"\x00") {
        name
    } else {
        name
    };
    // Safety: The function pointers should only be called by unsafe code that has to uphold their
    // representations in java code, which we presume to be accurate.
    unsafe {
        Some(match name {
            // SystemClassLoader
            b"Java_rho_SystemClassLoader_initializeSystemClassLoader" => {
                into_opaque2ret(system_class_loader::system_class_loader_init)
            }
            b"Java_rho_SystemClassLoader_getResources" => {
                into_opaque3ret(system_class_loader::system_class_loader_get_resources)
            }
            // ClassLoader
            b"Java_rho_SystemClassLoader_getSystemResourceAsStream" => into_opaque3ret(
                system_class_loader::system_class_loader_get_system_resouce_as_stream,
            ),
            // Object
            b"Java_java_lang_Object_getClass" => into_opaque2ret(object::object_get_class),
            b"Java_java_lang_Object_hashCode" => into_opaque2ret(object::object_hashcode),
            b"Java_java_lang_Object_clone" => into_opaque2ret(object::object_clone),
            // Class
            b"Java_java_lang_Class_getPrimitiveClass" => {
                into_opaque3ret(class::class_get_primitive)
            }
            b"Java_java_lang_Class_getClassForNameWithClassLoader" => {
                into_opaque5ret(class::class_get_class_for_name_with_class_loader)
            }
            b"Java_java_lang_Class_getClassForName" => {
                into_opaque3ret(class::class_get_class_for_name)
            }
            b"Java_java_lang_Class_getName" => into_opaque2ret(class::class_get_name),
            b"Java_java_lang_Class_getSimpleName" => into_opaque2ret(class::class_get_simple_name),
            b"Java_java_lang_Class_getPackage" => into_opaque2ret(class::class_get_package),
            b"Java_java_lang_Class_getDeclaredField" => {
                into_opaque3ret(class::class_get_declared_field)
            }
            b"Java_java_lang_Class_newInstance" => into_opaque2ret(class::class_new_instance),
            b"Java_java_lang_Class_isPrimitive" => into_opaque2ret(class::class_is_primitive),
            b"Java_java_lang_Class_isArray" => into_opaque2ret(class::class_is_array),
            b"Java_java_lang_Class_getComponentType" => {
                into_opaque2ret(class::class_get_component_type)
            }
            b"Java_java_lang_Class_isAssignableFrom" => {
                into_opaque3ret(class::class_is_assignable_from)
            }
            b"Java_java_lang_Class_isInstance" => into_opaque3ret(class::class_is_instance),
            b"Java_java_lang_Class_isInterface" => into_opaque2ret(class::class_is_interface),
            // reflect/Field
            b"Java_java_lang_reflect_Field_getType" => into_opaque2ret(field::field_get_type),
            // reflect/Array
            b"Java_java_lang_reflect_Array_newInstanceArray" => {
                into_opaque4ret(reflect_array::array_new_instance)
            }
            // rho/invoke/MethodHandle
            b"Java_rho_invoke_MethodHandle_invoke" => {
                into_opaque3ret(method_handle::method_handle_invoke)
            }
            // java/lang/invoke/MethodHandles
            b"Java_java_lang_invoke_MethodHandles_revealDirect" => {
                into_opaque3ret(method_handle::mh_lookup_reveal_direct)
            }
            // java/lang/invoke/MethodHandles$Lookup
            b"Java_java_lang_invoke_MethodHandles_00024Lookup_lookupClass" => {
                into_opaque2ret(method_handle::mhs_lookup_lookup_class)
            }
            // java/lang/invoke/MethodType
            b"Java_java_lang_invoke_MethodType_toMethodDescriptorString" => {
                into_opaque2ret(method_type::mt_to_method_descriptor_string)
            }
            // rho/invoke/MethodHandleInfoInst
            b"Java_rho_invoke_MethodHandleInfoInst_getDeclaringClass" => {
                into_opaque2ret(method_handle_info::mh_info_get_declaring_class)
            }
            b"Java_rho_invoke_MethodHandleInfoInst_getReferenceKind" => {
                into_opaque2ret(method_handle_info::mh_info_get_reference_kind)
            }
            b"Java_rho_invoke_MethodHandleInfoInst_getMethodType" => {
                into_opaque2ret(method_handle_info::mh_info_get_method_type)
            }
            b"Java_rho_invoke_MethodHandleInfoInst_getName" => {
                into_opaque2ret(method_handle_info::mh_info_get_name)
            }
            // System
            b"Java_java_lang_System_setProperties" => {
                into_opaque3ret(system::system_set_properties)
            }
            b"Java_java_lang_System_load" => into_opaque3ret(system::system_load),
            b"Java_java_lang_System_loadLibrary" => into_opaque3ret(system::system_load_library),
            b"Java_java_lang_System_arraycopy" => into_opaque7ret(system::system_arraycopy),
            b"Java_java_lang_System_currentTimeMillis" => {
                into_opaque2ret(system::system_current_time_milliseconds)
            }
            b"Java_java_lang_System_nanoTime" => into_opaque2ret(system::system_nano_time),
            // Runtime
            b"Java_java_lang_Runtime_availableProcessors" => {
                into_opaque2ret(runtime::runtime_available_processors)
            }
            b"Java_java_lang_Runtime_freeMemory" => into_opaque2ret(runtime::runtime_free_memory),
            b"Java_java_lang_Runtime_totalMemory" => into_opaque2ret(runtime::runtime_total_memory),
            b"Java_java_lang_Runtime_maxMemory" => into_opaque2ret(runtime::runtime_max_memory),
            // Primitive wrappers
            b"Java_java_lang_Float_floatToRawIntBits" => {
                into_opaque3ret(primitive::float_to_raw_int_bits)
            }
            b"Java_java_lang_Double_doubleToRawLongBits" => {
                into_opaque3ret(primitive::double_to_raw_long_bits)
            }
            b"Java_java_lang_Integer_numberOfLeadingZeros" => {
                into_opaque3ret(primitive::integer_number_of_leading_zeroes)
            }
            b"Java_java_lang_Integer_toString" => into_opaque4ret(primitive::integer_to_string),
            b"Java_java_lang_Integer_parseInt" => into_opaque4ret(primitive::integer_parse_int),
            b"Java_java_lang_Long_numberOfLeadingZeros" => {
                into_opaque3ret(primitive::long_number_of_leading_zeroes)
            }
            b"Java_java_lang_Long_toString" => into_opaque4ret(primitive::long_to_string),
            b"Java_java_lang_Long_parseInt" => into_opaque4ret(primitive::long_parse_int),
            // Unsafe allocation
            b"Java_sun_misc_Unsafe_allocateMemory" => {
                into_opaque3ret(unsafe_::unsafe_allocate_memory)
            }
            b"Java_sun_misc_Unsafe_freeMemory" => into_opaque3ret(unsafe_::unsafe_free_memory),
            // Unsafe get memory
            b"Java_sun_misc_Unsafe_getByte" => into_opaque3ret(unsafe_::unsafe_get_byte_ptr),
            b"Java_sun_misc_Unsafe_getShort" => into_opaque3ret(unsafe_::unsafe_get_short_ptr),
            b"Java_sun_misc_Unsafe_getChar" => into_opaque3ret(unsafe_::unsafe_get_char_ptr),
            b"Java_sun_misc_Unsafe_getInt" => into_opaque3ret(unsafe_::unsafe_get_int_ptr),
            b"Java_sun_misc_Unsafe_getLong" => into_opaque3ret(unsafe_::unsafe_get_long_ptr),
            b"Java_sun_misc_Unsafe_getFloat" => into_opaque3ret(unsafe_::unsafe_get_float_ptr),
            b"Java_sun_misc_Unsafe_getDouble" => into_opaque3ret(unsafe_::unsafe_get_double_ptr),
            // Unsafe put memory
            b"Java_sun_misc_Unsafe_putByte" => into_opaque4ret(unsafe_::unsafe_put_byte_ptr),
            b"Java_sun_misc_Unsafe_putShort" => into_opaque4ret(unsafe_::unsafe_put_short_ptr),
            b"Java_sun_misc_Unsafe_putChar" => into_opaque4ret(unsafe_::unsafe_put_char_ptr),
            b"Java_sun_misc_Unsafe_putInt" => into_opaque4ret(unsafe_::unsafe_put_int_ptr),
            b"Java_sun_misc_Unsafe_putLong" => into_opaque4ret(unsafe_::unsafe_put_long_ptr),
            b"Java_sun_misc_Unsafe_putFloat" => into_opaque4ret(unsafe_::unsafe_put_float_ptr),
            b"Java_sun_misc_Unsafe_putDouble" => into_opaque4ret(unsafe_::unsafe_put_double_ptr),
            // Unsafe get field
            b"Java_sun_misc_Unsafe_getObjectField" => into_opaque4ret(unsafe_::unsafe_get_object),
            b"Java_sun_misc_Unsafe_getIntField" => into_opaque4ret(unsafe_::unsafe_get_int),
            b"Java_sun_misc_Unsafe_getLongField" => into_opaque4ret(unsafe_::unsafe_get_long),
            // Unsafe put field
            b"Java_sun_misc_Unsafe_putObjectField" => into_opaque5ret(unsafe_::unsafe_put_object),
            b"Java_sun_misc_Unsafe_putIntField" => into_opaque5ret(unsafe_::unsafe_put_int),
            b"Java_sun_misc_Unsafe_putLongField" => into_opaque5ret(unsafe_::unsafe_put_long),
            // Unsafe fields
            b"Java_sun_misc_Unsafe_objectFieldOffset" => {
                into_opaque3ret(unsafe_::unsafe_object_field_offset)
            }
            b"Java_sun_misc_Unsafe_getAndAddInt" => {
                into_opaque5ret(unsafe_::unsafe_get_and_add_int)
            }

            // sun/reflect/Reflection
            b"Java_sun_reflect_Reflection_getCallerClass" => {
                into_opaque2ret(reflection::get_caller_class)
            }

            // Thread
            b"Java_java_lang_Thread_currentThread" => {
                into_opaque2ret(thread::thread_get_current_thread)
            }

            // String
            b"Java_java_lang_String_intern" => into_opaque2ret(string::string_intern),

            // UnsupportedOperationException
            b"Java_java_lang_UnsupportedOperationException_checkAbort" => {
                into_opaque2ret(unsupported_operation_exception_check_abort)
            }
            _ => return None,
        })
    }
}

extern "C" fn unsupported_operation_exception_check_abort(env: *mut Env<'_>, _this: JObject) {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    assert!(
        !env.state.conf().abort_on_unsupported,
        "UnsupportedOperationException"
    );
}
