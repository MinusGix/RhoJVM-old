use std::{
    ffi::{c_void, CStr},
    num::NonZeroUsize,
    os::raw::c_char,
};

use classfile_parser::method_info::MethodAccessFlags;
use rhojvm_base::{
    code::{
        method::{DescriptorType, DescriptorTypeBasic, Method, MethodDescriptor},
        types::{JavaChar, PrimitiveType},
    },
    id::ClassId,
    util::convert_classfile_text,
};
use smallvec::SmallVec;
use usize_cast::{IntoIsize, IntoUsize};

use crate::{
    class_instance::{
        ClassInstance, FieldIndex, Instance, PrimitiveArrayInstance, ReferenceArrayInstance,
        ReferenceInstance, StaticFormInstance,
    },
    eval::{
        eval_method, instances::make_instance_fields, EvalError, EvalMethodValue, Frame, Locals,
        ValueException,
    },
    gc::GcRef,
    initialize_class,
    jni::{self, OpaqueClassMethod},
    method::NativeMethod,
    rv::{RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    util::{
        construct_string, get_string_contents, get_string_contents_as_rust_string,
        make_class_form_of, Env,
    },
    GeneralError,
};

use super::{
    JArray, JBoolean, JByte, JByteArray, JChar, JClass, JDouble, JFieldId, JFloat, JInt, JLong,
    JMethodId, JObject, JObjectArray, JShort, JSize, JString, JThrowable, MethodNoArguments,
    Status,
};

macro_rules! unimpl_none_name {
    ($name:expr) => {{
        extern "C" fn unimpl_f(_: *mut Env) {
            panic!("Unimplemented Function: {}", $name);
        }
        unimpl_f
    }};
}

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

    pub new_global_ref: NewGlobalRefFn,
    pub delete_global_ref: DeleteGlobalRefFn,
    pub delete_local_ref: DeleteLocalRefFn,
    pub is_same_object: MethodNoArguments,
    pub new_local_ref: MethodNoArguments,
    pub ensure_local_capacity: EnsureLocalCapacityFn,

    pub alloc_object: MethodNoArguments,
    pub new_object: NewObjectFn,
    pub new_object_v: NewObjectVFn,
    pub new_object_a: MethodNoArguments,

    pub get_object_class: MethodNoArguments,
    pub is_instance_of: MethodNoArguments,

    pub get_method_id: GetMethodIdFn,

    pub call_object_method: CallObjectMethodFn,
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

    pub get_object_field: GetObjectFieldFn,
    pub get_boolean_field: GetBooleanFieldFn,
    pub get_byte_field: GetByteFieldFn,
    pub get_char_field: GetCharFieldFn,
    pub get_short_field: GetShortFieldFn,
    pub get_int_field: GetIntFieldFn,
    pub get_long_field: GetLongFieldFn,
    pub get_float_field: GetFloatFieldFn,
    pub get_double_field: GetDoubleFieldFn,
    pub set_object_field: SetObjectFieldFn,
    pub set_boolean_field: SetBooleanFieldFn,
    pub set_byte_field: SetByteFieldFn,
    pub set_char_field: SetCharFieldFn,
    pub set_short_field: SetShortFieldFn,
    pub set_int_field: SetIntFieldFn,
    pub set_long_field: SetLongFieldFn,
    pub set_float_field: SetFloatFieldFn,
    pub set_double_field: SetDoubleFieldFn,

    pub get_static_method_id: GetStaticMethodIdFn,

    pub call_static_object_method: MethodNoArguments,
    pub call_static_object_method_v: CallStaticObjectMethodVFn,
    pub call_static_object_method_a: MethodNoArguments,
    pub call_static_boolean_method: MethodNoArguments,
    pub call_static_boolean_method_v: CallStaticBooleanMethodVFn,
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

    pub get_static_field_id: GetStaticFieldIdFn,

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

    pub new_string: NewStringFn,

    pub get_string_length: GetStringLengthFn,
    pub get_string_chars: MethodNoArguments,
    pub release_string_chars: MethodNoArguments,

    pub new_string_utf: NewStringUtfFn,
    pub get_string_utf_length: MethodNoArguments,
    pub get_string_utf_chars: GetStringUtfCharsFn,
    pub release_string_utf_chars: ReleaseStringUtfCharsFn,

    pub get_array_length: GetArrayLengthFn,

    pub new_object_array: NewObjectArrayFn,
    pub get_object_array_element: GetObjectArrayElementFn,
    pub set_object_array_element: SetObjectArrayElementFn,

    pub new_boolean_array: MethodNoArguments,
    pub new_byte_array: NewByteArrayFn,
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
    pub get_byte_array_region: GetByteArrayRegionFn,
    pub get_char_array_region: MethodNoArguments,
    pub get_short_array_region: MethodNoArguments,
    pub get_int_array_region: MethodNoArguments,
    pub get_long_array_region: MethodNoArguments,
    pub get_float_array_region: MethodNoArguments,
    pub get_double_array_region: MethodNoArguments,
    pub set_boolean_array_region: MethodNoArguments,
    pub set_byte_array_region: SetByteArrayRegionFn,
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

    pub get_primitive_array_critical: GetPrimitiveArrayCriticalFn,
    pub release_primitive_array_critical: ReleasePrimitiveArrayCriticalFn,

    pub get_string_critical: GetStringCriticalFn,
    pub release_string_critical: ReleaseStringCriticalFn,

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
    pub fn new_typical() -> NativeInterface {
        NativeInterface {
            empty_0: unimpl_none_name!("empty_0"),
            empty_1: unimpl_none_name!("empty_1"),
            empty_2: unimpl_none_name!("empty_2"),
            empty_3: unimpl_none_name!("empty_3"),
            get_version,
            define_class,
            find_class,
            from_reflected_method: unimpl_none_name!("from_reflected_method"),
            from_reflected_field: unimpl_none_name!("from_reflected_field"),
            to_reflected_method: unimpl_none_name!("to_reflected_method"),
            get_superclass,
            is_assignable_from,
            to_reflected_field: unimpl_none_name!("to_reflected_field"),
            throw,
            throw_new,
            exception_occurred,
            exception_describe,
            exception_clear,
            fatal_error,
            push_local_frame: unimpl_none_name!("push_local_frame"),
            pop_local_frame: unimpl_none_name!("pop_local_frame"),
            new_global_ref,
            delete_global_ref,
            delete_local_ref,
            is_same_object: unimpl_none_name!("is_same_object"),
            new_local_ref: unimpl_none_name!("new_local_ref"),
            ensure_local_capacity,
            alloc_object: unimpl_none_name!("alloc_object"),
            new_object,
            new_object_v,
            new_object_a: unimpl_none_name!("new_object_a"),
            get_object_class: unimpl_none_name!("get_object_class"),
            is_instance_of: unimpl_none_name!("is_instance_of"),
            get_method_id,
            call_object_method,
            call_object_method_v: unimpl_none_name!("call_object_method_v"),
            call_object_method_a: unimpl_none_name!("call_object_method_a"),
            call_boolean_method: unimpl_none_name!("call_boolean_method"),
            call_boolean_method_v: unimpl_none_name!("call_boolean_method_v"),
            call_boolean_method_a: unimpl_none_name!("call_boolean_method_a"),
            call_byte_method: unimpl_none_name!("call_byte_method"),
            call_byte_method_v: unimpl_none_name!("call_byte_method_v"),
            call_byte_method_a: unimpl_none_name!("call_byte_method_a"),
            call_char_method: unimpl_none_name!("call_char_method"),
            call_char_method_v: unimpl_none_name!("call_char_method_v"),
            call_char_method_a: unimpl_none_name!("call_char_method_a"),
            call_short_method: unimpl_none_name!("call_short_method"),
            call_short_method_v: unimpl_none_name!("call_short_method_v"),
            call_short_method_a: unimpl_none_name!("call_short_method_a"),
            call_int_method: unimpl_none_name!("call_int_method"),
            call_int_method_v: unimpl_none_name!("call_int_method_v"),
            call_int_method_a: unimpl_none_name!("call_int_method_a"),
            call_long_method: unimpl_none_name!("call_long_method"),
            call_long_method_v: unimpl_none_name!("call_long_method_v"),
            call_long_method_a: unimpl_none_name!("call_long_method_a"),
            call_float_method: unimpl_none_name!("call_float_method"),
            call_float_method_v: unimpl_none_name!("call_float_method_v"),
            call_float_method_a: unimpl_none_name!("call_float_method_a"),
            call_double_method: unimpl_none_name!("call_double_method"),
            call_double_method_v: unimpl_none_name!("call_double_method_v"),
            call_double_method_a: unimpl_none_name!("call_double_method_a"),
            call_void_method: unimpl_none_name!("call_void_method"),
            call_void_method_v: unimpl_none_name!("call_void_method_v"),
            call_void_method_a: unimpl_none_name!("call_void_method_a"),
            call_nonvirtual_object_method: unimpl_none_name!("call_nonvirtual_object_method"),
            call_nonvirtual_object_method_v: unimpl_none_name!("call_nonvirtual_object_method_v"),
            call_nonvirtual_object_method_a: unimpl_none_name!("call_nonvirtual_object_method_a"),
            call_nonvirtual_boolean_method: unimpl_none_name!("call_nonvirtual_boolean_method"),
            call_nonvirtual_boolean_method_v: unimpl_none_name!("call_nonvirtual_boolean_method_v"),
            call_nonvirtual_boolean_method_a: unimpl_none_name!("call_nonvirtual_boolean_method_a"),
            call_nonvirtual_byte_method: unimpl_none_name!("call_nonvirtual_byte_method"),
            call_nonvirtual_byte_method_v: unimpl_none_name!("call_nonvirtual_byte_method_v"),
            call_nonvirtual_byte_method_a: unimpl_none_name!("call_nonvirtual_byte_method_a"),
            call_nonvirtual_char_method: unimpl_none_name!("call_nonvirtual_char_method"),
            call_nonvirtual_char_method_v: unimpl_none_name!("call_nonvirtual_char_method_v"),
            call_nonvirtual_char_method_a: unimpl_none_name!("call_nonvirtual_char_method_a"),
            call_nonvirtual_short_method: unimpl_none_name!("call_nonvirtual_short_method"),
            call_nonvirtual_short_method_v: unimpl_none_name!("call_nonvirtual_short_method_v"),
            call_nonvirtual_short_method_a: unimpl_none_name!("call_nonvirtual_short_method_a"),
            call_nonvirtual_int_method: unimpl_none_name!("call_nonvirtual_int_method"),
            call_nonvirtual_int_method_v: unimpl_none_name!("call_nonvirtual_int_method_v"),
            call_nonvirtual_int_method_a: unimpl_none_name!("call_nonvirtual_int_method_a"),
            call_nonvirtual_long_method: unimpl_none_name!("call_nonvirtual_long_method"),
            call_nonvirtual_long_method_v: unimpl_none_name!("call_nonvirtual_long_method_v"),
            call_nonvirtual_long_method_a: unimpl_none_name!("call_nonvirtual_long_method_a"),
            call_nonvirtual_float_method: unimpl_none_name!("call_nonvirtual_float_method"),
            call_nonvirtual_float_method_v: unimpl_none_name!("call_nonvirtual_float_method_v"),
            call_nonvirtual_float_method_a: unimpl_none_name!("call_nonvirtual_float_method_a"),
            call_nonvirtual_double_method: unimpl_none_name!("call_nonvirtual_double_method"),
            call_nonvirtual_double_method_v: unimpl_none_name!("call_nonvirtual_double_method_v"),
            call_nonvirtual_double_method_a: unimpl_none_name!("call_nonvirtual_double_method_a"),
            call_nonvirtual_void_method: unimpl_none_name!("call_nonvirtual_void_method"),
            call_nonvirtual_void_method_v: unimpl_none_name!("call_nonvirtual_void_method_v"),
            call_nonvirtual_void_method_a: unimpl_none_name!("call_nonvirtual_void_method_a"),
            get_field_id,
            get_object_field,
            get_boolean_field,
            get_byte_field,
            get_char_field,
            get_short_field,
            get_int_field,
            get_long_field,
            get_float_field,
            get_double_field,
            set_object_field,
            set_boolean_field,
            set_byte_field,
            set_char_field,
            set_short_field,
            set_int_field,
            set_long_field,
            set_float_field,
            set_double_field,
            get_static_method_id,
            call_static_object_method: unimpl_none_name!("call_static_object_method"),
            call_static_object_method_v,
            call_static_object_method_a: unimpl_none_name!("call_static_object_method_a"),
            call_static_boolean_method: unimpl_none_name!("call_static_boolean_method"),
            call_static_boolean_method_v,
            call_static_boolean_method_a: unimpl_none_name!("call_static_boolean_method_a"),
            call_static_byte_method: unimpl_none_name!("call_static_byte_method"),
            call_static_byte_method_v: unimpl_none_name!("call_static_byte_method_v"),
            call_static_byte_method_a: unimpl_none_name!("call_static_byte_method_a"),
            call_static_char_method: unimpl_none_name!("call_static_char_method"),
            call_static_char_method_v: unimpl_none_name!("call_static_char_method_v"),
            call_static_char_method_a: unimpl_none_name!("call_static_char_method_a"),
            call_static_short_method: unimpl_none_name!("call_static_short_method"),
            call_static_short_method_v: unimpl_none_name!("call_static_short_method_v"),
            call_static_short_method_a: unimpl_none_name!("call_static_short_method_a"),
            call_static_int_method: unimpl_none_name!("call_static_int_method"),
            call_static_int_method_v: unimpl_none_name!("call_static_int_method_v"),
            call_static_int_method_a: unimpl_none_name!("call_static_int_method_a"),
            call_static_long_method: unimpl_none_name!("call_static_long_method"),
            call_static_long_method_v: unimpl_none_name!("call_static_long_method_v"),
            call_static_long_method_a: unimpl_none_name!("call_static_long_method_a"),
            call_static_float_method: unimpl_none_name!("call_static_float_method"),
            call_static_float_method_v: unimpl_none_name!("call_static_float_method_v"),
            call_static_float_method_a: unimpl_none_name!("call_static_float_method_a"),
            call_static_double_method: unimpl_none_name!("call_static_double_method"),
            call_static_double_method_v: unimpl_none_name!("call_static_double_method_v"),
            call_static_double_method_a: unimpl_none_name!("call_static_double_method_a"),
            call_static_void_method: unimpl_none_name!("call_static_void_method"),
            call_static_void_method_v: unimpl_none_name!("call_static_void_method_v"),
            call_static_void_method_a: unimpl_none_name!("call_static_void_method_a"),

            get_static_field_id,
            get_static_object_field: unimpl_none_name!("get_static_object_field"),
            get_static_boolean_field: unimpl_none_name!("get_static_boolean_field"),
            get_static_byte_field: unimpl_none_name!("get_static_byte_field"),
            get_static_char_field: unimpl_none_name!("get_static_char_field"),
            get_static_short_field: unimpl_none_name!("get_static_short_field"),
            get_static_int_field: unimpl_none_name!("get_static_int_field"),
            get_static_long_field: unimpl_none_name!("get_static_long_field"),
            get_static_float_field: unimpl_none_name!("get_static_float_field"),
            get_static_double_field: unimpl_none_name!("get_static_double_field"),
            set_static_object_field: unimpl_none_name!("set_static_object_field"),
            set_static_boolean_field: unimpl_none_name!("set_static_boolean_field"),
            set_static_byte_field: unimpl_none_name!("set_static_byte_field"),
            set_static_char_field: unimpl_none_name!("set_static_char_field"),
            set_static_short_field: unimpl_none_name!("set_static_short_field"),
            set_static_int_field: unimpl_none_name!("set_static_int_field"),
            set_static_long_field: unimpl_none_name!("set_static_long_field"),
            set_static_float_field: unimpl_none_name!("set_static_float_field"),
            set_static_double_field: unimpl_none_name!("set_static_double_field"),

            new_string,
            get_string_length,
            get_string_chars: unimpl_none_name!("get_string_chars"),
            release_string_chars: unimpl_none_name!("release_string_chars"),
            new_string_utf,
            get_string_utf_length: unimpl_none_name!("get_string_utf_length"),
            get_string_utf_chars,
            release_string_utf_chars,
            get_array_length,
            new_object_array,
            get_object_array_element,
            set_object_array_element,
            new_boolean_array: unimpl_none_name!("new_boolean_array"),
            new_byte_array,
            new_char_array: unimpl_none_name!("new_char_array"),
            new_short_array: unimpl_none_name!("new_short_array"),
            new_int_array: unimpl_none_name!("new_int_array"),
            new_long_array: unimpl_none_name!("new_long_array"),
            new_float_array: unimpl_none_name!("new_float_array"),
            new_double_array: unimpl_none_name!("new_double_array"),
            get_boolean_array_elements: unimpl_none_name!("get_boolean_array_elements"),
            get_byte_array_elements: unimpl_none_name!("get_byte_array_elements"),
            get_char_array_elements: unimpl_none_name!("get_char_array_elements"),
            get_short_array_elements: unimpl_none_name!("get_short_array_elements"),
            get_int_array_elements: unimpl_none_name!("get_int_array_elements"),
            get_long_array_elements: unimpl_none_name!("get_long_array_elements"),
            get_float_array_elements: unimpl_none_name!("get_float_array_elements"),
            get_double_array_elements: unimpl_none_name!("get_double_array_elements"),
            release_boolean_array_elements: unimpl_none_name!("release_boolean_array_elements"),
            release_byte_array_elements: unimpl_none_name!("release_byte_array_elements"),
            release_char_array_elements: unimpl_none_name!("release_char_array_elements"),
            release_short_array_elements: unimpl_none_name!("release_short_array_elements"),
            release_int_array_elements: unimpl_none_name!("release_int_array_elements"),
            release_long_array_elements: unimpl_none_name!("release_long_array_elements"),
            release_float_array_elements: unimpl_none_name!("release_float_array_elements"),
            release_double_array_elements: unimpl_none_name!("release_double_array_elements"),
            get_boolean_array_region: unimpl_none_name!("get_boolean_array_region"),
            get_byte_array_region,
            get_char_array_region: unimpl_none_name!("get_char_array_region"),
            get_short_array_region: unimpl_none_name!("get_short_array_region"),
            get_int_array_region: unimpl_none_name!("get_int_array_region"),
            get_long_array_region: unimpl_none_name!("get_long_array_region"),
            get_float_array_region: unimpl_none_name!("get_float_array_region"),
            get_double_array_region: unimpl_none_name!("get_double_array_region"),
            set_boolean_array_region: unimpl_none_name!("set_boolean_array_region"),
            set_byte_array_region,
            set_char_array_region: unimpl_none_name!("set_char_array_region"),
            set_short_array_region: unimpl_none_name!("set_short_array_region"),
            set_int_array_region: unimpl_none_name!("set_int_array_region"),
            set_long_array_region: unimpl_none_name!("set_long_array_region"),
            set_float_array_region: unimpl_none_name!("set_float_array_region"),
            set_double_array_region: unimpl_none_name!("set_double_array_region"),
            register_natives,
            unregister_natives: unimpl_none_name!("unregister_natives"),
            monitor_enter: unimpl_none_name!("monitor_enter"),
            monitor_exit: unimpl_none_name!("monitor_exit"),
            get_java_vm: unimpl_none_name!("get_java_vm"),
            get_string_region: unimpl_none_name!("get_string_region"),
            get_string_utf_region: unimpl_none_name!("get_string_utf_region"),
            get_primitive_array_critical,
            release_primitive_array_critical,
            get_string_critical,
            release_string_critical,
            new_weak_global_ref: unimpl_none_name!("new_weak_global_ref"),
            delete_weak_global_ref: unimpl_none_name!("delete_weak_global_ref"),
            exception_check,
            new_direct_byte_buffer: unimpl_none_name!("new_direct_byte_buffer"),
            get_direct_buffer_address: unimpl_none_name!("get_direct_buffer_address"),
            get_direct_buffer_capacity: unimpl_none_name!("get_direct_buffer_capacity"),
            get_object_ref_type: unimpl_none_name!("get_object_ref_type"),
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

fn unimpl(message: &str) -> ! {
    panic!("Unimplemented Function: {}", message);
}

fn assert_valid_env(env: *const Env) {
    assert!(!env.is_null(), "Native method's env was a nullptr");
}

fn assert_non_aliasing<T, U>(l: *const T, r: *const U) {
    assert!(l as usize != r as usize, "Two pointers that should not alias in a native method aliased. This might be indicative of a bug within the JVM where it should allow that rather than a bug with the calling code");
}

// TODO: Type-safe versions of these function for Rust usage

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

pub type FindClassFn = unsafe extern "C" fn(env: *mut Env, name: *const c_char) -> JClass;
/// This is defined to be in the format that we expect
extern "C" fn find_class(env: *mut Env, name: *const c_char) -> JClass {
    assert_valid_env(env);
    assert_non_aliasing(env, name);

    let env = unsafe { &mut *env };

    assert!(
        !name.is_null(),
        "FindClass method was passed in a null name ptr",
    );

    // Safety: We know it is not null, but all we can really do is trust the caller
    // For whether it is valid data or not.
    let name = unsafe { CStr::from_ptr(name) };

    // TODO: We currently don't use any other class loaders

    let name = name.to_bytes();

    let class_id = env.class_names.gcid_from_bytes(name);

    // TODO: Use proper from class or use different method of creating it.
    let static_form = make_class_form_of(env, class_id, class_id);
    match static_form {
        // TODO: This might throw too many kinds of exceptions?
        Ok(v) => {
            if let Some(value) = env.state.extract_value(v) {
                unsafe { env.get_local_jobject_for(value.into_generic()) }
            } else {
                JClass::null()
            }
        }
        Err(err) => {
            tracing::warn!("FindClass error: {:?}", err);
            JClass::null()
        }
    }
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
    assert_valid_env(env);
    let env = unsafe { &mut *env };

    // TODO: assert that the obj is actually throwable

    let obj = unsafe { env.get_jobject_as_gcref(obj) };
    if let Some(obj) = obj {
        let obj = obj.unchecked_as();
        env.state.fill_native_exception(obj);
        return 0;
    }

    panic!("trying to throw null reference");
}

pub type ThrowNewFn =
    unsafe extern "C" fn(env: *mut Env, class: JClass, message: *const JChar) -> JInt;
extern "C" fn throw_new(env: *mut Env, class: JClass, message: *const JChar) -> JInt {
    unimpl("ThrowNew")
}

pub type ExceptionOccurredFn = unsafe extern "C" fn(env: *mut Env) -> JThrowable;
extern "C" fn exception_occurred(env: *mut Env) -> JThrowable {
    assert_valid_env(env);
    let env = unsafe { &mut *env };

    if let Some(exc) = env.state.native_exception {
        unsafe { env.get_local_jobject_for(exc.into_generic()) }
    } else {
        JThrowable::null()
    }
}

pub type ExceptionDescribeFn = unsafe extern "C" fn(env: *mut Env);
extern "C" fn exception_describe(env: *mut Env) {
    unimpl("ExceptionDescribe")
}

pub type ExceptionClearFn = unsafe extern "C" fn(env: *mut Env);
extern "C" fn exception_clear(env: *mut Env) {
    assert_valid_env(env);
    let env = unsafe { &mut *env };

    let _ = env.state.native_exception.take();
}

pub type FatalErrorFn = unsafe extern "C" fn(env: *mut Env, msg: *const JChar);
extern "C" fn fatal_error(env: *mut Env, msg: *const JChar) {
    // TODO: log message
    panic!("Fatal Error");
}

pub type ExceptionCheckFn = unsafe extern "C" fn(env: *mut Env) -> JBoolean;
extern "C" fn exception_check(env: *mut Env) -> JBoolean {
    assert_valid_env(env);
    let env = unsafe { &mut *env };

    u8::from(env.state.native_exception.is_some())
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
    assert_valid_env(env);
    assert_non_aliasing(env, methods);

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

    // Safety: No other thread should be using this
    // Though this relies on the native code being valid.
    // We already assert that it is not null
    let env = &mut *env;

    // Safety: We assume that it is a valid ref and that it has not been
    // forged.
    let class = env
        .get_jobject_as_gcref(class)
        .expect("RegisterNative's class was null");

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
        // We've already checked that it is non-null and non-directly-aliasing with env
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
    assert_valid_env(env);
    // Name and signature can alias, if in some absurd scenario that happens
    assert_non_aliasing(env, name);
    assert_non_aliasing(env, signature);

    assert!(!name.is_null(), "GetFieldId received nullptr name");
    assert!(
        !signature.is_null(),
        "GetFieldId received nullptr signature"
    );

    // Safety: We asserted that it is non-null
    let env = &mut *env;

    let class = env
        .get_jobject_as_gcref(class)
        .expect("GetFieldId's class was null");
    // The class id of the class we were given\
    let class_id = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(class).unwrap()
    {
        this.of
            .into_reference()
            .expect("Expected Class<T> of a Class")
    } else {
        // TODO: Don't panic
        panic!();
    };

    // Safety of both of these:
    // We've already checked that it is non-null and non-directly-aliasing with env
    // Since we aren't calling back into C code between here and the return, then they won't change
    // out from under us.
    // However, we have no guarantee that these actually end in a null-byte
    // And, we have no guarantee that their length is < isize::MAX
    // TODO: Checked cstr constructor
    let name = CStr::from_ptr(name);
    let signature = CStr::from_ptr(signature);

    let name_bytes = name.to_bytes();
    let signature_bytes = signature.to_bytes();

    match get_field_id_safe(env, name_bytes, signature_bytes, class_id) {
        Ok(value) => match value {
            ValueException::Value(field_index) => JFieldId::new_unchecked(class_id, field_index),
            ValueException::Exception(_exc) => {
                todo!("Handle exception properly for GetFieldID");
                JFieldId::null()
            }
        },
        // TODO: Handle errors better
        Err(err) => {
            panic!("Handle error properly for GetFieldID: {:?}", err);
            JFieldId::null()
        }
    }
}

fn get_field_for<'a>(env: &'a mut Env, obj: JObject, field_id: JFieldId) -> &'a mut RuntimeValue {
    let obj = unsafe { env.get_jobject_as_gcref(obj) }.expect("Object was null");
    let field_id = unsafe { field_id.into_field_id() }.expect("Null field id");

    let obj_instance = env.state.gc.deref_mut(obj).expect("Bad gc ref");
    let field = match obj_instance {
        Instance::StaticClass(_) => panic!("Static class ref is not allowed"),
        Instance::Reference(re) => re.get_class_fields_mut().unwrap().get_mut(field_id),
    };

    let field = field.expect("Failed to find field");
    field.value_mut()
}

pub type GetObjectFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId) -> JObject;
unsafe extern "C" fn get_object_field(env: *mut Env, obj: JObject, field_id: JFieldId) -> JObject {
    assert_valid_env(env);
    let env = &mut *env;
    match *get_field_for(env, obj, field_id) {
        RuntimeValue::Primitive(_) => panic!("Field was a primitive"),
        RuntimeValue::NullReference => JObject::null(),
        RuntimeValue::Reference(re) => env.get_local_jobject_for(re.into_generic()),
    }
}

pub type GetBooleanFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId) -> JBoolean;
unsafe extern "C" fn get_boolean_field(
    env: *mut Env,
    obj: JObject,
    field_id: JFieldId,
) -> JBoolean {
    assert_valid_env(env);
    let env = &mut *env;
    if let RuntimeValue::Primitive(RuntimeValuePrimitive::Bool(value)) =
        *get_field_for(env, obj, field_id)
    {
        value.into()
    } else {
        panic!("Field did not contain a bool");
    }
}

pub type GetByteFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId) -> JByte;
unsafe extern "C" fn get_byte_field(env: *mut Env, obj: JObject, field_id: JFieldId) -> JByte {
    assert_valid_env(env);
    let env = &mut *env;
    if let RuntimeValue::Primitive(RuntimeValuePrimitive::I8(value)) =
        *get_field_for(env, obj, field_id)
    {
        value
    } else {
        panic!("Field did not contain a byte");
    }
}

pub type GetCharFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId) -> JChar;
unsafe extern "C" fn get_char_field(env: *mut Env, obj: JObject, field_id: JFieldId) -> JChar {
    assert_valid_env(env);
    let env = &mut *env;
    if let RuntimeValue::Primitive(RuntimeValuePrimitive::Char(value)) =
        *get_field_for(env, obj, field_id)
    {
        value.as_i16() as u16
    } else {
        panic!("Field did not contain a char");
    }
}

pub type GetShortFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId) -> JShort;
unsafe extern "C" fn get_short_field(env: *mut Env, obj: JObject, field_id: JFieldId) -> JShort {
    assert_valid_env(env);
    let env = &mut *env;
    if let RuntimeValue::Primitive(RuntimeValuePrimitive::I16(value)) =
        *get_field_for(env, obj, field_id)
    {
        value
    } else {
        panic!("Field did not contain a short");
    }
}

pub type GetIntFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId) -> JInt;
unsafe extern "C" fn get_int_field(env: *mut Env, obj: JObject, field_id: JFieldId) -> JInt {
    assert_valid_env(env);
    let env = &mut *env;
    if let RuntimeValue::Primitive(RuntimeValuePrimitive::I32(value)) =
        *get_field_for(env, obj, field_id)
    {
        value
    } else {
        panic!("Field did not contain an int");
    }
}

pub type GetLongFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId) -> JLong;
unsafe extern "C" fn get_long_field(env: *mut Env, obj: JObject, field_id: JFieldId) -> JLong {
    assert_valid_env(env);
    let env = &mut *env;
    if let RuntimeValue::Primitive(RuntimeValuePrimitive::I64(value)) =
        *get_field_for(env, obj, field_id)
    {
        value
    } else {
        panic!("Field did not contain a long");
    }
}

pub type GetFloatFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId) -> JFloat;
unsafe extern "C" fn get_float_field(env: *mut Env, obj: JObject, field_id: JFieldId) -> JFloat {
    assert_valid_env(env);
    let env = &mut *env;
    if let RuntimeValue::Primitive(RuntimeValuePrimitive::F32(value)) =
        *get_field_for(env, obj, field_id)
    {
        value
    } else {
        panic!("Field did not contain a float");
    }
}

pub type GetDoubleFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId) -> JDouble;
unsafe extern "C" fn get_double_field(env: *mut Env, obj: JObject, field_id: JFieldId) -> JDouble {
    assert_valid_env(env);
    let env = &mut *env;
    if let RuntimeValue::Primitive(RuntimeValuePrimitive::F64(value)) =
        *get_field_for(env, obj, field_id)
    {
        value
    } else {
        panic!("Field did not contain a double");
    }
}

pub type SetObjectFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId, value: JObject);
unsafe extern "C" fn set_object_field(
    env: *mut Env,
    obj: JObject,
    field_id: JFieldId,
    value: JObject,
) {
    assert_valid_env(env);
    let env = &mut *env;
    // FIXME: Check that we can set that object into the field
    let value = env.get_jobject_as_gcref(value);
    let field_value = get_field_for(env, obj, field_id);

    if let Some(value) = value {
        *field_value = RuntimeValue::Reference(value.unchecked_as());
    } else {
        *field_value = RuntimeValue::NullReference;
    }
}
pub type SetBooleanFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId, value: JBoolean);
unsafe extern "C" fn set_boolean_field(
    env: *mut Env,
    obj: JObject,
    field_id: JFieldId,
    value: JBoolean,
) {
    assert_valid_env(env);
    let env = &mut *env;
    let field_value = get_field_for(env, obj, field_id);
    *field_value = RuntimeValuePrimitive::Bool(value).into();
}
pub type SetByteFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId, value: JByte);
unsafe extern "C" fn set_byte_field(env: *mut Env, obj: JObject, field_id: JFieldId, value: JByte) {
    assert_valid_env(env);
    let env = &mut *env;
    let field_value = get_field_for(env, obj, field_id);
    *field_value = RuntimeValuePrimitive::I8(value).into();
}
pub type SetCharFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId, value: JChar);
unsafe extern "C" fn set_char_field(env: *mut Env, obj: JObject, field_id: JFieldId, value: JChar) {
    assert_valid_env(env);
    let env = &mut *env;
    let value = JavaChar(value as u16);
    let field_value = get_field_for(env, obj, field_id);
    *field_value = RuntimeValuePrimitive::Char(value).into();
}
pub type SetShortFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId, value: JShort);
unsafe extern "C" fn set_short_field(
    env: *mut Env,
    obj: JObject,
    field_id: JFieldId,
    value: JShort,
) {
    assert_valid_env(env);
    let env = &mut *env;
    let field_value = get_field_for(env, obj, field_id);
    *field_value = RuntimeValuePrimitive::I16(value).into();
}
pub type SetIntFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId, value: JInt);
unsafe extern "C" fn set_int_field(env: *mut Env, obj: JObject, field_id: JFieldId, value: JInt) {
    assert_valid_env(env);
    let env = &mut *env;
    let field_value = get_field_for(env, obj, field_id);
    *field_value = RuntimeValuePrimitive::I32(value).into();
}
pub type SetLongFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId, value: JLong);
unsafe extern "C" fn set_long_field(env: *mut Env, obj: JObject, field_id: JFieldId, value: JLong) {
    assert_valid_env(env);
    let env = &mut *env;
    let field_value = get_field_for(env, obj, field_id);
    *field_value = RuntimeValuePrimitive::I64(value).into();
}
pub type SetFloatFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId, value: JFloat);
unsafe extern "C" fn set_float_field(
    env: *mut Env,
    obj: JObject,
    field_id: JFieldId,
    value: JFloat,
) {
    assert_valid_env(env);
    let env = &mut *env;
    let field_value = get_field_for(env, obj, field_id);
    *field_value = RuntimeValuePrimitive::F32(value).into();
}
pub type SetDoubleFieldFn =
    unsafe extern "C" fn(env: *mut Env, obj: JObject, field_id: JFieldId, value: JDouble);
unsafe extern "C" fn set_double_field(
    env: *mut Env,
    obj: JObject,
    field_id: JFieldId,
    value: JDouble,
) {
    assert_valid_env(env);
    let env = &mut *env;
    let field_value = get_field_for(env, obj, field_id);
    *field_value = RuntimeValuePrimitive::F64(value).into();
}

fn get_field_id_safe(
    env: &mut Env,
    name: &[u8],
    signature: &[u8],
    class_id: ClassId,
) -> Result<ValueException<FieldIndex>, GeneralError> {
    // TODO: Don't unwrap
    let class_file = env
        .class_files
        .get(&class_id)
        .ok_or(GeneralError::MissingLoadedClassFile(class_id))?;

    // Note: GetFieldId can't be used to get the length field of an array
    for (field_index, field_data) in class_file.load_field_values_iter().enumerate() {
        let field_index = FieldIndex::new_unchecked(field_index as u16);
        let (field_info, _) = field_data.map_err(GeneralError::ClassFileLoad)?;
        let target_field_name = class_file.get_text_b(field_info.name_index).ok_or(
            EvalError::InvalidConstantPoolIndex(field_info.name_index.into_generic()),
        )?;

        // If their names are unequal then simply skip it
        if name != target_field_name {
            continue;
        }

        // desc/signature are essentially the same thing, just a bit of a mixed up terminology
        let target_field_desc = class_file.get_text_b(field_info.descriptor_index).ok_or(
            EvalError::InvalidConstantPoolIndex(field_info.descriptor_index.into_generic()),
        )?;

        // Linear compare is probably faster than parsing and I don't think we need to do any
        // typecasting?
        // TODO: Though, we could provide some warnings anyway?
        if signature == target_field_desc {
            // We've found it, so we can simply return here.
            return Ok(ValueException::Value(field_index));
        }
    }

    todo!("Return NoSuchFieldException")
}

pub type GetArrayLengthFn = unsafe extern "C" fn(env: *mut Env, instance: JArray) -> JSize;
unsafe extern "C" fn get_array_length(env: *mut Env, instance: JArray) -> JSize {
    assert_valid_env(env);
    let env = &mut *env;

    let instance = env
        .get_jobject_as_gcref(instance)
        .expect("GetByteArrayRegion was null ref");
    let instance = env
        .state
        .gc
        .deref(instance)
        .expect("Failed to get instance");

    match instance {
        Instance::StaticClass(_) => panic!("Got static class instance in get array length"),
        Instance::Reference(re) => match re {
            ReferenceInstance::StaticForm(_)
            | ReferenceInstance::Class(_)
            | ReferenceInstance::Thread(_)
            | ReferenceInstance::MethodHandle(_)
            | ReferenceInstance::MethodHandleInfo(_) => panic!("Got class"),
            ReferenceInstance::PrimitiveArray(arr) => arr.len(),
            ReferenceInstance::ReferenceArray(arr) => arr.len(),
        },
    }
}

pub type NewObjectArrayFn = unsafe extern "C" fn(
    env: *mut Env,
    length: JSize,
    class: JClass,
    initial_element: JObject,
) -> JObjectArray;
unsafe extern "C" fn new_object_array(
    env: *mut Env,
    length: JSize,
    class: JClass,
    initial_element: JObject,
) -> JObjectArray {
    assert_valid_env(env);
    let env = &mut *env;

    if length < 0 {
        todo!("Throw NegativeArraySizeException");
    }

    let length = length as u32;
    let length = length.into_usize();

    let class = unsafe { env.get_jobject_as_gcref(class) }.expect("Class was null ref");
    let class = class.unchecked_as::<StaticFormInstance>();
    let class_id = env
        .state
        .gc
        .deref(class)
        .unwrap()
        .of
        .into_reference()
        .unwrap();

    let initial_element = unsafe { env.get_jobject_as_gcref(initial_element) };
    let initial_element = initial_element.map(GcRef::unchecked_as::<ReferenceInstance>);

    let array_id = env
        .class_names
        .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), class_id)
        .unwrap();

    let arr = ReferenceArrayInstance::new(array_id, class_id, vec![initial_element; length]);

    let arr_ref = env.state.gc.alloc(arr);

    unsafe { env.get_local_jobject_for(arr_ref.into_generic()) }
}

pub type GetObjectArrayElementFn =
    unsafe extern "C" fn(env: *mut Env, array: JObjectArray, index: JSize) -> JObject;
unsafe extern "C" fn get_object_array_element(
    env: *mut Env,
    array: JObjectArray,
    index: JSize,
) -> JObject {
    assert_valid_env(env);
    let env = &mut *env;

    if index < 0 {
        todo!("Throw ArrayIndexOutOfBoundsException");
    }

    let index = index as u32;
    let index = index.into_usize();

    let array = env
        .get_jobject_as_gcref(array)
        .expect("GetObjectArrayElement was null ref");
    let array = array.unchecked_as::<ReferenceArrayInstance>();

    let array = env.state.gc.deref(array).expect("Failed to get instance");

    let element = array.elements.get(index).expect("Failed to get element");

    let element = element.map(|e| unsafe { env.get_local_jobject_for(e.into_generic()) });

    element.unwrap_or(JObject::null())
}

pub type SetObjectArrayElementFn =
    unsafe extern "C" fn(env: *mut Env, array: JObjectArray, index: JSize, value: JObject);
unsafe extern "C" fn set_object_array_element(
    env: *mut Env,
    array: JObjectArray,
    index: JSize,
    value: JObject,
) {
    assert_valid_env(env);
    let env = &mut *env;

    if index < 0 {
        todo!("Throw ArrayIndexOutOfBoundsException");
    }

    let index = index as u32;
    let index = index.into_usize();

    let value = unsafe { env.get_jobject_as_gcref(value) };
    let value = value.map(GcRef::unchecked_as::<ReferenceInstance>);

    let array = env
        .get_jobject_as_gcref(array)
        .expect("GetObjectArrayElement was null ref");
    let array = array.unchecked_as::<ReferenceArrayInstance>();

    let array = env
        .state
        .gc
        .deref_mut(array)
        .expect("Failed to get instance");

    array.elements[index] = value;
}

pub type NewByteArrayFn = unsafe extern "C" fn(env: *mut Env, length: JSize) -> JByteArray;
unsafe extern "C" fn new_byte_array(env: *mut Env, length: JSize) -> JByteArray {
    assert_valid_env(env);
    let env = &mut *env;

    let byte_array_id = env
        .class_names
        .gcid_from_array_of_primitives(PrimitiveType::UnsignedByte);

    let elements: Vec<RuntimeValuePrimitive> = {
        assert!(length >= 0, "Negative length");

        let length = length.unsigned_abs();
        let length = length.into_usize();

        vec![RuntimeValuePrimitive::I8(0); length]
    };

    let arr = PrimitiveArrayInstance::new(byte_array_id, RuntimeTypePrimitive::I8, elements);
    let arr = env.state.gc.alloc(arr);
    unsafe { env.get_local_jobject_for(arr.into_generic()) }
}

pub type GetByteArrayRegionFn = unsafe extern "C" fn(
    env: *mut Env,
    instance: JByteArray,
    start: JSize,
    length: JSize,
    output: *mut JByte,
);
unsafe extern "C" fn get_byte_array_region(
    env: *mut Env,
    instance: JByteArray,
    start: JSize,
    length: JSize,
    output: *mut JByte,
) {
    assert_valid_env(env);
    assert!(!output.is_null(), "output buffer was a nullptr");
    let env = &mut *env;

    assert!(start >= 0, "Negative start");

    let start = start.unsigned_abs();
    let start = start.into_usize();

    assert!(length >= 0, "Negative length");

    let length = length.unsigned_abs();
    let length = length.into_usize();

    let end = if let Some(end) = start.checked_add(length) {
        end
    } else {
        panic!("length + start would overflow");
    };

    let instance = env
        .get_jobject_as_gcref(instance)
        .expect("GetByteArrayRegion was null ref");
    let instance = env
        .state
        .gc
        .deref(instance)
        .expect("Failed to get instance");
    if let Instance::Reference(ReferenceInstance::PrimitiveArray(instance)) = instance {
        assert!(instance.element_type == RuntimeTypePrimitive::I8);
        assert!(isize::try_from(instance.elements.len()).is_ok());

        assert!(
            start < instance.elements.len(),
            "Start is past end of array"
        );

        assert!(end <= instance.elements.len(), "End is past end of array");

        let iter = instance
            .elements
            .iter()
            .skip(start)
            .take(length)
            .enumerate();

        for (offset, val) in iter {
            let ptr_dest = output.add(offset);
            if let RuntimeValuePrimitive::I8(val) = val {
                *ptr_dest = *val;
            } else {
                panic!("Bad value in i8 array");
            }
        }
    } else {
        panic!("Instance was not a primitive array")
    }
}
pub type SetByteArrayRegionFn = unsafe extern "C" fn(
    env: *mut Env,
    instance: JByteArray,
    start: JSize,
    length: JSize,
    buffer: *const JByte,
);
unsafe extern "C" fn set_byte_array_region(
    env: *mut Env,
    instance: JByteArray,
    start: JSize,
    length: JSize,
    buffer: *const JByte,
) {
    assert_valid_env(env);
    assert!(!buffer.is_null(), "input buffer was a nullptr");
    let env = &mut *env;

    assert!(start >= 0, "Negative start");

    let start = start.unsigned_abs();
    let start = start.into_usize();

    assert!(length >= 0, "Negative length");

    let length = length.unsigned_abs();
    let length = length.into_usize();

    let end = if let Some(end) = start.checked_add(length) {
        end
    } else {
        panic!("length + start would overflow");
    };

    let instance_ref = env
        .get_jobject_as_gcref(instance)
        .expect("SetByteArrayRegion array was null");
    let instance = env
        .state
        .gc
        .deref_mut(instance_ref)
        .expect("Failed to get array");
    if let Instance::Reference(ReferenceInstance::PrimitiveArray(instance)) = instance {
        assert!(instance.element_type == RuntimeTypePrimitive::I8);
        assert!(isize::try_from(instance.elements.len()).is_ok());

        assert!(
            start < instance.elements.len(),
            "Start is past end of array"
        );

        assert!(end <= instance.elements.len(), "End is past end of array");

        let iter = instance
            .elements
            .iter_mut()
            .skip(start)
            .take(length)
            .enumerate();
        for (offset, val) in iter {
            let ptr_src = buffer.add(offset);
            let src = *ptr_src;
            *val = RuntimeValuePrimitive::I8(src);
        }
    } else {
        panic!("Instance was not a primitive array");
    }
}

pub type DeleteLocalRefFn = unsafe extern "C" fn(env: *mut Env, obj: JObject);
unsafe extern "C" fn delete_local_ref(env: *mut Env, obj: JObject) {
    assert_valid_env(env);
    // TODO: Implement this.
}

pub type EnsureLocalCapacityFn = unsafe extern "C" fn(env: *mut Env, capacity: JInt) -> JInt;
unsafe extern "C" fn ensure_local_capacity(env: *mut Env, capacity: JInt) -> JInt {
    assert_valid_env(env);

    // TODO: We should be more explicit about what assurances we actually provide rather than just
    // saying that we can allocate how many instances it wants.

    Status::Ok as JInt
}

pub type NewObjectFn =
    unsafe extern "C" fn(env: *mut Env, class: JClass, method_id: JMethodId, ...) -> JObject;
unsafe extern "C" fn new_object(
    env: *mut Env,
    class: JClass,
    method_id: JMethodId,
    mut args: ...
) -> JObject {
    let args = args.as_va_list();

    new_object_v(env, class, method_id, args)
}

pub type NewObjectVFn = unsafe extern "C" fn(
    env: *mut Env,
    class: JClass,
    method_id: JMethodId,
    args: std::ffi::VaList,
) -> JObject;
unsafe extern "C" fn new_object_v(
    env: *mut Env,
    class: JClass,
    method_id: JMethodId,
    mut args: std::ffi::VaList,
) -> JObject {
    assert_valid_env(env);
    let env = &mut *env;

    let static_form = env
        .get_jobject_as_gcref(class)
        .expect("NewObject class was null")
        .unchecked_as::<StaticFormInstance>();
    let static_form = env.state.gc.deref(static_form).unwrap();
    let of = static_form.of.into_reference().unwrap();

    tracing::info!(
        "native_interface: new_object of={} ({:?})",
        env.class_names.tpath(of),
        of
    );

    if let Ok(true) = env.class_names.is_array(of) {
        panic!("Cannot call new_object on an array class");
    }

    // TODO: This is INVALID for classes that we manage internally!!
    let fields = make_instance_fields(env, of).unwrap();
    let Some(fields) = env.state.extract_value(fields) else {
        return JObject::null();
    };
    let static_ref = initialize_class(env, of).unwrap().into_value();
    let Some(static_ref) = env.state.extract_value(static_ref) else {
        return JObject::null();
    };
    let instance = ClassInstance::new(of, static_ref, fields);
    let instance_ref = env.state.gc.alloc(instance);

    let method_id = method_id.into_method_id().unwrap();
    let method = env.methods.get(&method_id).unwrap();
    let desc_parameters: SmallVec<[_; 8]> =
        method.descriptor().parameters().iter().copied().collect();
    let mut locals = Locals::new_with_array([RuntimeValue::Reference(instance_ref.into_generic())]);
    for param in desc_parameters {
        let value: RuntimeValue = promote_param(env, &mut args, param);

        locals.push_transform(value);
    }

    let frame = Frame::new_locals(locals);

    match eval_method(env, method_id.into(), frame).unwrap() {
        EvalMethodValue::ReturnVoid => unsafe {
            env.get_local_jobject_for(instance_ref.into_generic())
        },
        EvalMethodValue::Return(_) => {
            panic!("Constructor returned a value!");
        }
        EvalMethodValue::Exception(exc) => {
            env.state.fill_native_exception(exc);
            JObject::null()
        }
    }
}

pub type NewGlobalRefFn = unsafe extern "C" fn(env: *mut Env, obj: JObject) -> JObject;
unsafe extern "C" fn new_global_ref(env: *mut Env, obj: JObject) -> JObject {
    assert_valid_env(env);
    // FIXME: Currently we don't inform the garbage collector that they're pinned
    // because we ignore the gc.

    tracing::warn!("new_global_ref called, but is not implemented properly");

    obj
}

pub type DeleteGlobalRefFn = unsafe extern "C" fn(env: *mut Env, obj: JObject);
unsafe extern "C" fn delete_global_ref(env: *mut Env, obj: JObject) {
    assert_valid_env(env);
}

pub type GetStaticFieldIdFn = unsafe extern "C" fn(
    env: *mut Env,
    class: JClass,
    name: *const c_char,
    signature: *const c_char,
) -> JFieldId;
unsafe extern "C" fn get_static_field_id(
    env: *mut Env,
    class: JClass,
    name: *const c_char,
    signature: *const c_char,
) -> JFieldId {
    assert_valid_env(env);

    assert_non_aliasing(env, name);
    assert_non_aliasing(env, signature);

    // Safety: We asserted that it is non-null
    let env = &mut *env;

    let class = env
        .get_jobject_as_gcref(class)
        .expect("GetStaticFieldId class was null")
        .unchecked_as::<StaticFormInstance>();

    let class = env.state.gc.deref(class).unwrap();
    let class_id = class.of.into_reference().unwrap();

    let name = CStr::from_ptr(name);
    let signature = CStr::from_ptr(signature);

    let name_bytes = name.to_bytes();
    let signature_bytes = signature.to_bytes();

    match get_field_id_safe(env, name_bytes, signature_bytes, class_id) {
        Ok(value) => match value {
            ValueException::Value(field_index) => JFieldId::new_unchecked(class_id, field_index),
            ValueException::Exception(_exc) => {
                todo!("Handle exception properly for GetStaticFieldID");
                JFieldId::null()
            }
        },
        // TODO: Handle errors better
        Err(err) => {
            panic!("Handle error properly for GetStaticFieldID: {:?}", err);
            JFieldId::null()
        }
    }
}

pub type NewStringFn =
    unsafe extern "C" fn(env: *mut Env, chars: *const JChar, len: JSize) -> JString;
unsafe extern "C" fn new_string(env: *mut Env, chars: *const JChar, len: JSize) -> JString {
    assert_valid_env(env);
    assert!(!chars.is_null(), "New String chars ptr was null");

    if len < 0 {
        // The docs don't say what to do if this is negative
        tracing::error!("Negative length string");
        return JString::null();
    }
    let len = len.unsigned_abs().into_usize();

    assert_non_aliasing(env, chars);

    let env = &mut *env;

    let mut content = Vec::with_capacity(len);
    for i in 0..len {
        let char_at = chars.add(i);
        let char_at = *char_at;
        let char_at = JavaChar(char_at);
        content.push(RuntimeValuePrimitive::Char(char_at));
    }

    let text = construct_string(env, content).unwrap();
    if let Some(text) = env.state.extract_value(text) {
        env.get_local_jobject_for(text.into_generic())
    } else {
        // Exception
        JString::null()
    }
}

pub type NewStringUtfFn = unsafe extern "C" fn(env: *mut Env, chars: *const c_char) -> JString;
unsafe extern "C" fn new_string_utf(env: *mut Env, chars: *const c_char) -> JString {
    assert_valid_env(env);
    assert!(!chars.is_null(), "New String chars ptr was null");
    assert_non_aliasing(env, chars);

    let env = &mut *env;

    let chars = CStr::from_ptr(chars);
    let chars = chars.to_bytes();
    // TODO: Convert directly to utf16
    // The text is in 'modified utf8 encoding' aka cesu8
    let chars = convert_classfile_text(chars);
    let content = chars
        .encode_utf16()
        .map(|x| RuntimeValuePrimitive::Char(JavaChar(x)))
        .collect();

    let text = construct_string(env, content).unwrap();
    if let Some(text) = env.state.extract_value(text) {
        env.get_local_jobject_for(text.into_generic())
    } else {
        // Exception
        JString::null()
    }
}

pub type GetStringUtfCharsFn =
    unsafe extern "C" fn(env: *mut Env, string: JString, is_copy: *mut JBoolean) -> *const c_char;
unsafe extern "C" fn get_string_utf_chars(
    env: *mut Env,
    string: JString,
    is_copy: *mut JBoolean,
) -> *const c_char {
    assert_valid_env(env);
    let env = &mut *env;

    let string = unsafe { env.get_jobject_as_gcref(string) };
    let string = string.expect("TODO: NPE");

    // TODO: This should be Cesu8?
    let string = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        string,
    )
    .unwrap();

    let string = string.into_bytes();

    let layout = std::alloc::Layout::array::<c_char>(string.len() + 1).unwrap();
    let output: *mut u8 = std::alloc::alloc_zeroed(layout);
    assert!(!output.is_null(), "Failed to allocate memory for string");

    let output: *mut c_char = output as *mut c_char;

    for (i, v) in string.iter().enumerate() {
        let output = output.add(i);
        *output = *v as c_char;
    }

    if !is_copy.is_null() {
        *is_copy = JBoolean::from(true);
    }

    output
}

pub type ReleaseStringUtfCharsFn =
    unsafe extern "C" fn(env: *mut Env, string: JString, chars: *const c_char);
unsafe extern "C" fn release_string_utf_chars(
    env: *mut Env,
    string: JString,
    chars: *const c_char,
) {
    assert_valid_env(env);
    let env = &mut *env;

    // TODO: We could guard against invalid pointers

    // We have to reget the string to get the correct layout.
    // TODO: Is this valid? What if the string has changed?
    let string = unsafe { env.get_jobject_as_gcref(string) };
    let string = string.expect("TODO: NPE");

    let string = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        string,
    )
    .unwrap();

    let string = string.into_bytes();

    let layout = std::alloc::Layout::array::<c_char>(string.len() + 1).unwrap();
    std::alloc::dealloc(chars as *mut u8, layout);
}

pub type GetStringLengthFn = unsafe extern "C" fn(env: *mut Env, string: JString) -> JSize;
unsafe extern "C" fn get_string_length(env: *mut Env, string: JString) -> JSize {
    assert_valid_env(env);
    let env = &mut *env;

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    if let Some(string) = env.get_jobject_as_gcref(string) {
        get_string_contents(
            &env.class_files,
            &mut env.class_names,
            &mut env.state,
            string,
        )
        .unwrap()
        .len() as i32
    } else {
        todo!("NPE?");
    }
}

pub type GetMethodIdFn = unsafe extern "C" fn(
    env: *mut Env,
    this: JClass,
    name: *const c_char,
    signature: *const c_char,
) -> JMethodId;
unsafe extern "C" fn get_method_id(
    env: *mut Env,
    this: JClass,
    name: *const c_char,
    signature: *const c_char,
) -> JMethodId {
    // FIXME: This currently does not support special methods like array clone!
    get_jmethod_id(env, this, name, signature, |method| {
        #[allow(clippy::manual_assert)]
        if method.access_flags().contains(MethodAccessFlags::STATIC) {
            // TODO: Throw no such method error
            panic!("Found method but it was static");
        } else {
            None
        }
    })
}

pub type GetStaticMethodIdFn = unsafe extern "C" fn(
    env: *mut Env,
    this: JClass,
    name: *const c_char,
    signature: *const c_char,
) -> JMethodId;
unsafe extern "C" fn get_static_method_id(
    env: *mut Env,
    this: JClass,
    name: *const c_char,
    signature: *const c_char,
) -> JMethodId {
    get_jmethod_id(env, this, name, signature, |method| {
        #[allow(clippy::manual_assert)]
        if method.access_flags().contains(MethodAccessFlags::STATIC) {
            None
        } else {
            // TODO: Throw no such method error
            panic!("Found method but it wasn't static");
        }
    })
}

unsafe fn get_jmethod_id(
    env: *mut Env,
    this: JClass,
    name: *const c_char,
    signature: *const c_char,
    validate: impl FnOnce(&Method) -> Option<JMethodId>,
) -> JMethodId {
    assert_valid_env(env);
    assert!(!name.is_null(), "get method id's name was null");
    assert!(!signature.is_null(), "get method id's signature was null",);
    assert_non_aliasing(env, name);
    assert_non_aliasing(env, signature);
    // name and signature can alias if they wish

    let env = &mut *env;

    // Get the class id for the class that Class<T> holds
    let this = env.get_jobject_as_gcref(this);
    let this = this.expect("GetStaticMethod's class was null");
    let this_id = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        this.of
            .into_reference()
            .expect("Expected Class<T> of a class")
    } else {
        // This should be caught by method calling
        // Though it would be good to not panic
        panic!();
    };

    // Get CStr then just immediately convert to a slice of bytes because they're cesu8
    // and so we can use that directly
    let name = CStr::from_ptr(name);
    let name = name.to_bytes();
    let signature = CStr::from_ptr(signature);
    let signature = signature.to_bytes();

    let desc = MethodDescriptor::from_text(signature, &mut env.class_names).unwrap();

    // TODO: Throw no such method error
    // TODO: Can you have a static and instance method with the same name and would this be
    // confused by that?
    let method_id = env
        .methods
        .load_method_from_desc(
            &mut env.class_names,
            &mut env.class_files,
            this_id,
            name,
            &desc,
        )
        .unwrap();

    let method = env.methods.get(&method_id).unwrap();
    if let Some(method_id) = validate(method) {
        method_id
    } else {
        let (class_id, method_index) = method_id.decompose();

        JMethodId::new_unchecked(class_id, method_index)
    }
}

unsafe fn promote_param(
    env: &mut Env,
    args: &mut std::ffi::VaList,
    param: DescriptorType,
) -> RuntimeValue {
    match param {
        DescriptorType::Basic(basic) => match basic {
            DescriptorTypeBasic::Byte => RuntimeValuePrimitive::I8(args.arg::<JByte>()).into(),
            DescriptorTypeBasic::Char => {
                RuntimeValuePrimitive::Char(JavaChar(args.arg::<JChar>())).into()
            }
            DescriptorTypeBasic::Double => RuntimeValuePrimitive::F64(args.arg::<JDouble>()).into(),
            // It seems like C variadics promote f32 to f64, so we have to get the f64 then
            // cast down
            DescriptorTypeBasic::Float =>
            {
                #[allow(clippy::cast_possible_truncation)]
                RuntimeValuePrimitive::F32(args.arg::<JDouble>() as JFloat).into()
            }
            DescriptorTypeBasic::Int => RuntimeValuePrimitive::I32(args.arg::<JInt>()).into(),
            DescriptorTypeBasic::Long => RuntimeValuePrimitive::I64(args.arg::<JLong>()).into(),
            DescriptorTypeBasic::Short => RuntimeValuePrimitive::I16(args.arg::<JShort>()).into(),
            DescriptorTypeBasic::Boolean => {
                RuntimeValuePrimitive::Bool(args.arg::<JBoolean>()).into()
            }
            DescriptorTypeBasic::Class(_) => {
                // Requires JObject to be transparent, which it is
                let arg = args.arg::<*const ()>();
                let arg = JObject::new_unchecked(arg);
                if let Some(arg) = env.get_jobject_as_gcref(arg) {
                    assert!(
                        env.state.gc.deref(arg).is_some(),
                        "Invalid GcRef ({:?}) gotten from variadic list",
                        arg
                    );
                    // TODO: Check that the type is valid
                    RuntimeValue::Reference(arg.unchecked_as())
                } else {
                    RuntimeValue::NullReference
                }
            }
        },
        DescriptorType::Array { level, component } => {
            if level.get() > 1 {
                todo!("Arrays of arrays");
            }

            let arg = args.arg::<*const ()>();
            let arg = JObject::new_unchecked(arg);
            if let Some(arg) = env.get_jobject_as_gcref(arg) {
                assert!(
                    env.state.gc.deref(arg).is_some(),
                    "Invalid GcRef ({:?}) gotten from variadic list",
                    arg
                );
                // TODO: Check that the type is valid
                RuntimeValue::Reference(arg.unchecked_as())
            } else {
                RuntimeValue::NullReference
            }
        }
    }
}

pub type CallObjectMethodFn =
    unsafe extern "C" fn(env: *mut Env, object: JObject, method_id: JMethodId, ...) -> JObject;
unsafe extern "C" fn call_object_method(
    env: *mut Env,
    object: JObject,
    method_id: JMethodId,
    mut args: ...
) -> JObject {
    assert_valid_env(env);
    let env = &mut *env;

    let object = env.get_jobject_as_gcref(object);
    let object = object.unwrap();

    let mut args = args.as_va_list();

    let method_id = method_id.into_method_id().unwrap();
    let method = env.methods.get(&method_id).unwrap();
    // Have to allocate due to needing env mutably in loop
    let desc_parameters: SmallVec<[_; 8]> =
        method.descriptor().parameters().iter().copied().collect();
    let mut locals = Locals::new_with_array([RuntimeValue::Reference(object.unchecked_as())]);
    for param in desc_parameters {
        let value: RuntimeValue = promote_param(env, &mut args, param);

        locals.push_transform(value);
    }

    let frame = Frame::new_locals(locals);

    match eval_method(env, method_id.into(), frame).unwrap() {
        EvalMethodValue::ReturnVoid => {
            panic!("Method which was intended to return an object returned void")
        }
        EvalMethodValue::Return(value) => {
            if let Some(value) = value.into_reference().expect("Returned non referenced") {
                env.get_local_jobject_for(value.into_generic())
            } else {
                JObject::null()
            }
        }
        EvalMethodValue::Exception(exc) => {
            env.state.fill_native_exception(exc);
            JObject::null()
        }
    }
}

pub type CallStaticObjectMethodVFn = unsafe extern "C" fn(
    env: *mut Env,
    class: JClass,
    method_id: JMethodId,
    args: std::ffi::VaList,
) -> JObject;
unsafe extern "C" fn call_static_object_method_v(
    env: *mut Env,
    _class: JClass,
    method_id: JMethodId,
    mut args: std::ffi::VaList,
) -> JObject {
    assert_valid_env(env);
    let env = &mut *env;

    let method_id = method_id.into_method_id().unwrap();
    let method = env.methods.get(&method_id).unwrap();
    // Have to allocate due to needing env mutably in loop
    let desc_parameters: SmallVec<[_; 8]> =
        method.descriptor().parameters().iter().copied().collect();
    let mut locals = Locals::default();
    for param in desc_parameters {
        let value: RuntimeValue = promote_param(env, &mut args, param);

        locals.push_transform(value);
    }

    let frame = Frame::new_locals(locals);

    match eval_method(env, method_id.into(), frame).unwrap() {
        EvalMethodValue::ReturnVoid => {
            panic!("Static method which was intended to return an object returned void")
        }
        EvalMethodValue::Return(value) => {
            if let Some(value) = value.into_reference().expect("Returned non referenced") {
                env.get_local_jobject_for(value.into_generic())
            } else {
                JObject::null()
            }
        }
        EvalMethodValue::Exception(exc) => {
            env.state.fill_native_exception(exc);
            JObject::null()
        }
    }
}

pub type CallStaticBooleanMethodVFn = unsafe extern "C" fn(
    env: *mut Env,
    class: JClass,
    method_id: JMethodId,
    args: std::ffi::VaList,
) -> JBoolean;
unsafe extern "C" fn call_static_boolean_method_v(
    env: *mut Env,
    _class: JClass,
    method_id: JMethodId,
    mut args: std::ffi::VaList,
) -> JBoolean {
    assert_valid_env(env);
    let env = &mut *env;

    let method_id = method_id.into_method_id().unwrap();
    let method = env.methods.get(&method_id).unwrap();
    // Have to allocate due to needing env mutably in loop
    let desc_parameters: SmallVec<[_; 8]> =
        method.descriptor().parameters().iter().copied().collect();
    let mut locals = Locals::default();
    for param in desc_parameters {
        let value: RuntimeValue = promote_param(env, &mut args, param);

        locals.push_transform(value);
    }

    let frame = Frame::new_locals(locals);

    match eval_method(env, method_id.into(), frame).unwrap() {
        EvalMethodValue::ReturnVoid => {
            panic!("Static method which was intended to return a boolean returned void")
        }
        EvalMethodValue::Return(value) => value.into_bool_loose().unwrap(),
        EvalMethodValue::Exception(exc) => {
            env.state.fill_native_exception(exc);
            0
        }
    }
}

pub type GetStringCriticalFn =
    unsafe extern "C" fn(env: *mut Env, string: JString, is_copy: *mut JBoolean) -> *const JChar;
unsafe extern "C" fn get_string_critical(
    env: *mut Env,
    string: JString,
    is_copy: *mut JBoolean,
) -> *const JChar {
    assert_valid_env(env);
    assert_non_aliasing(env, is_copy);

    let env = &mut *env;

    // TODO: Currently we can't directly give a pointer because we've wrapped primitive arrays
    // in runtimevalueprimitive which is obviously not transparent
    // However, for getstringcritical, we can - for perf - pass a direct pointer
    // since it can not issue jni calls while it has the pointer
    // Though, we'd probably want to still copy by default

    // TODO: We could mark a critical mode on env, which panics if this thread calls any
    // jni functions? Or at least have it in a debug mode?

    if let Some(string) = env.get_jobject_as_gcref(string) {
        let data = get_string_contents(
            &env.class_files,
            &mut env.class_names,
            &mut env.state,
            string,
        )
        .unwrap();
        let len = data.len();
        let data = data.iter().map(|x| match x {
            RuntimeValuePrimitive::Char(v) => v.0,
            _ => unreachable!(),
        });

        // I didn't see a good way to do this with Box and vec/slices?
        // Like, I could get Box<[u16]> and get *const u16, but
        // it wouldn't necessarily be valid to use it in from_raw..
        // which is what I need.
        // Thus we allocate manually
        let layout = std::alloc::Layout::array::<JChar>(len).unwrap();
        let output = std::alloc::alloc_zeroed(layout);
        assert!(
            !output.is_null(),
            "Failed to allocate space string in GetStringCritical",
        );
        // Layout should have made this aligned
        #[allow(clippy::cast_ptr_alignment)]
        let output = output.cast::<JChar>();
        for (i, x) in data.enumerate() {
            let output_dest = output.add(i);
            *output_dest = x;
        }

        if !is_copy.is_null() {
            *is_copy = JBoolean::from(true);
        }

        // Safety: This won't be deallocated until we get it back later
        output
    } else {
        todo!("NPE");
    }
}
pub type ReleaseStringCriticalFn =
    unsafe extern "C" fn(env: *mut Env, string: JString, data_ptr: *const JChar);
unsafe extern "C" fn release_string_critical(
    env: *mut Env,
    string: JString,
    data_ptr: *const JChar,
) {
    assert_valid_env(env);
    assert_non_aliasing(env, data_ptr);
    assert!(!data_ptr.is_null());

    let env = &mut *env;

    // TODO: We could keep track, at least in debug mode or with safety settings enabled,
    // whether the pointer is valid and whether it has already been freed
    // to guard against uaf and invalid ptrs

    // We're assuming that computing the same layout gives the same result
    if let Some(string) = env.get_jobject_as_gcref(string) {
        let data = get_string_contents(
            &env.class_files,
            &mut env.class_names,
            &mut env.state,
            string,
        )
        .unwrap();
        let len = data.len();
        let data = data.iter().map(|x| match x {
            RuntimeValuePrimitive::Char(v) => v.0,
            _ => unreachable!(),
        });

        let layout = std::alloc::Layout::array::<JChar>(len).unwrap();
        // It feels a bit iffy to cast it back to a mut pointer so it can be deallocated, but
        // that's how this api works
        std::alloc::dealloc(data_ptr as *mut u8, layout);
    } else {
        todo!("NPE");
    }
}

fn construct_layout_for_primitive_array(
    typ: RuntimeTypePrimitive,
    len: usize,
) -> std::alloc::Layout {
    use std::alloc::Layout;
    match typ {
        RuntimeTypePrimitive::I64 => Layout::array::<JLong>(len),
        RuntimeTypePrimitive::I32 => Layout::array::<JInt>(len),
        RuntimeTypePrimitive::I16 => Layout::array::<JShort>(len),
        RuntimeTypePrimitive::I8 => Layout::array::<JByte>(len),
        RuntimeTypePrimitive::Bool => Layout::array::<JBoolean>(len),
        RuntimeTypePrimitive::F32 => Layout::array::<JFloat>(len),
        RuntimeTypePrimitive::F64 => Layout::array::<JDouble>(len),
        RuntimeTypePrimitive::Char => Layout::array::<JChar>(len),
    }
    .unwrap()
}

unsafe fn copy_values<T>(target_ptr: *mut T, values: impl Iterator<Item = T>) {
    for (i, val) in values.enumerate() {
        assert!(isize::try_from(i).is_ok());
        let element_ptr = target_ptr.add(i);
        std::ptr::write(element_ptr, val);
    }
}

fn allocate_primitive_array(
    typ: RuntimeTypePrimitive,
    data: &[RuntimeValuePrimitive],
) -> *mut c_void {
    let layout = construct_layout_for_primitive_array(typ, data.len());
    let ptr = unsafe { std::alloc::alloc_zeroed(layout) };

    let data = data.iter().copied();
    unsafe {
        match typ {
            RuntimeTypePrimitive::I64 => {
                copy_values(ptr.cast(), data.map(|x| x.into_i64().unwrap()));
            }
            RuntimeTypePrimitive::I32 => {
                copy_values(ptr.cast(), data.map(|x| x.into_i32().unwrap()));
            }
            RuntimeTypePrimitive::I16 => {
                copy_values(ptr.cast(), data.map(|x| x.into_i16().unwrap()));
            }
            RuntimeTypePrimitive::I8 => {
                copy_values(ptr.cast(), data.map(|x| x.into_byte().unwrap()));
            }
            // This is a bit iffy. We're copying as bytes.
            RuntimeTypePrimitive::Bool => {
                copy_values(ptr.cast(), data.map(|x| x.into_bool_loose().unwrap()));
            }
            RuntimeTypePrimitive::F32 => {
                copy_values(ptr.cast(), data.map(|x| x.into_f32().unwrap()));
            }
            RuntimeTypePrimitive::F64 => {
                copy_values(ptr.cast(), data.map(|x| x.into_f64().unwrap()));
            }
            RuntimeTypePrimitive::Char => {
                copy_values(ptr.cast(), data.map(|x| x.into_char().unwrap().0));
            }
        }
    }

    ptr.cast()
}

pub type GetPrimitiveArrayCriticalFn =
    unsafe extern "C" fn(env: *mut Env, array: JArray, is_copy: *mut JBoolean) -> *mut c_void;
unsafe extern "C" fn get_primitive_array_critical(
    env: *mut Env,
    array: JArray,
    is_copy: *mut JBoolean,
) -> *mut c_void {
    assert_valid_env(env);
    assert_non_aliasing(env, is_copy);

    let env = &mut *env;

    if let Some(array) = env.get_jobject_as_gcref(array) {
        // NOTE: We can't just return a pointer to the data even once we get support for that
        // because GetPrimitiveArrayCritical can be called multiple times
        // However, ensuring that that is right and that they don't use the same array is hard
        // so it may be best not to ever return a raw pointer.
        // It might theoretically be fine, even if they're the same instance, since we aren't
        // forging references from them, but it is still iffy

        let array = env
            .state
            .gc
            .deref(array.unchecked_as::<ReferenceInstance>())
            .unwrap();
        match array {
            ReferenceInstance::PrimitiveArray(array) => {
                allocate_primitive_array(array.element_type, &array.elements)
            }
            ReferenceInstance::ReferenceArray(_) => todo!(),
            // Invalid
            ReferenceInstance::Class(_) => todo!(),
            ReferenceInstance::StaticForm(_) => todo!(),
            ReferenceInstance::Thread(_) => todo!(),
            ReferenceInstance::MethodHandle(_) => todo!(),
            ReferenceInstance::MethodHandleInfo(_) => todo!(),
        }
    } else {
        todo!("NPE")
    }
}

pub type ReleasePrimitiveArrayCriticalFn =
    unsafe extern "C" fn(env: *mut Env, array: JArray, ptr: *mut c_void, mode: JInt);
unsafe extern "C" fn release_primitive_array_critical(
    env: *mut Env,
    array: JArray,
    ptr: *mut c_void,
    mode: JInt,
) {
    // FIXME: We need to copy the data back into the array since there might have been edits!
    tracing::warn!("ReleasePrimitiveArrayCritical unimplemented");
}
