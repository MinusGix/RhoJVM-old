//! Internal replacements for native functions  

use crate::{
    jni::{JObject, MethodClassNoArguments, OpaqueClassMethod},
    util::Env,
};

mod class;
mod object;
mod primitive;
mod system;
mod system_class_loader;
mod unsafe_;

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
            // Object
            b"Java_java_lang_Object_getClass" => into_opaque2ret(object::object_get_class),
            b"Java_java_lang_Object_hashCode" => into_opaque2ret(object::object_hashcode),
            // Class
            b"Java_java_lang_Class_getPrimitive" => into_opaque3ret(class::class_get_primitive),
            b"Java_java_lang_Class_getClassForNameWithClassLoader" => {
                into_opaque5ret(class::class_get_class_for_name_with_class_loader)
            }
            b"Java_java_lang_Class_getClassForName" => {
                into_opaque3ret(class::class_get_class_for_name)
            }
            b"Java_java_lang_Class_getDeclaredField" => {
                into_opaque3ret(class::class_get_declared_field)
            }
            b"Java_java_lang_Class_newInstance" => into_opaque2ret(class::class_new_instance),
            // System
            b"Java_java_lang_System_setProperties" => {
                into_opaque3ret(system::system_set_properties)
            }
            b"Java_java_lang_System_arraycopy" => into_opaque7ret(system::system_arraycopy),
            // Primitive wrappers
            b"Java_java_lang_Float_floatToRawIntBits" => {
                into_opaque3ret(primitive::float_to_raw_int_bits)
            }
            b"Java_java_lang_Double_doubleToRawLongBits" => {
                into_opaque3ret(primitive::double_to_raw_long_bits)
            }
            b"Java_java_lang_Integer_toString" => into_opaque4ret(primitive::integer_to_string),
            b"Java_java_lang_Integer_parseInt" => into_opaque4ret(primitive::integer_parse_int),
            // Unsafe allocation
            b"Java_sun_misc_Unsafe_allocateMemory" => {
                into_opaque3ret(unsafe_::unsafe_allocate_memory)
            }
            b"Java_sun_misc_Unsafe_freeMemory" => into_opaque3ret(unsafe_::unsafe_free_memory),
            // Unsafe get
            b"Java_sun_misc_Unsafe_getByte" => into_opaque3ret(unsafe_::unsafe_get_byte),
            b"Java_sun_misc_Unsafe_getShort" => into_opaque3ret(unsafe_::unsafe_get_short),
            b"Java_sun_misc_Unsafe_getChar" => into_opaque3ret(unsafe_::unsafe_get_char),
            b"Java_sun_misc_Unsafe_getInt" => into_opaque3ret(unsafe_::unsafe_get_int),
            b"Java_sun_misc_Unsafe_getLong" => into_opaque3ret(unsafe_::unsafe_get_long),
            b"Java_sun_misc_Unsafe_getFloat" => into_opaque3ret(unsafe_::unsafe_get_float),
            b"Java_sun_misc_Unsafe_getDouble" => into_opaque3ret(unsafe_::unsafe_get_double),
            // Unsafe put
            b"Java_sun_misc_Unsafe_putByte" => into_opaque4ret(unsafe_::unsafe_put_byte),
            b"Java_sun_misc_Unsafe_putShort" => into_opaque4ret(unsafe_::unsafe_put_short),
            b"Java_sun_misc_Unsafe_putChar" => into_opaque4ret(unsafe_::unsafe_put_char),
            b"Java_sun_misc_Unsafe_putInt" => into_opaque4ret(unsafe_::unsafe_put_int),
            b"Java_sun_misc_Unsafe_putLong" => into_opaque4ret(unsafe_::unsafe_put_long),
            b"Java_sun_misc_Unsafe_putFloat" => into_opaque4ret(unsafe_::unsafe_put_float),
            b"Java_sun_misc_Unsafe_putDouble" => into_opaque4ret(unsafe_::unsafe_put_double),
            // Unsafe fields
            b"Java_sun_misc_Unsafe_objectFieldOffset" => {
                into_opaque3ret(unsafe_::unsafe_object_field_offset)
            }
            b"Java_sun_misc_Unsafe_getAndAddInt" => {
                into_opaque5ret(unsafe_::unsafe_get_and_add_int)
            }
            _ => return None,
        })
    }
}
