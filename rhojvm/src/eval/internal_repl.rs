//! Internal replacements for native functions  

use classfile_parser::field_info::FieldAccessFlags;
use either::Either;
use rhojvm_base::{code::types::JavaChar, id::ClassId};
use usize_cast::IntoUsize;

use crate::{
    class_instance::{
        ClassInstance, FieldIndex, Instance, PrimitiveArrayInstance, ReferenceInstance,
    },
    eval::{instances::make_fields, ValueException},
    gc::GcRef,
    initialize_class,
    jni::{
        JChar, JDouble, JFieldId, JFloat, JInt, JLong, JObject, JString, MethodClassNoArguments,
        OpaqueClassMethod,
    },
    rv::{RuntimeValue, RuntimeValuePrimitive},
    util::{self, find_field_with_name, Env},
};

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
            b"Java_java_lang_Object_getClass" => into_opaque2ret(object_get_class),
            b"Java_java_lang_Object_hashCode" => into_opaque2ret(object_hashcode),
            b"Java_java_lang_Class_getPrimitive" => into_opaque3ret(class_get_primitive),
            b"Java_java_lang_Class_getDeclaredField" => into_opaque3ret(class_get_declared_field),
            b"Java_java_lang_System_arraycopy" => into_opaque7ret(system_arraycopy),
            b"Java_java_lang_Float_floatToRawIntBits" => into_opaque3ret(float_to_raw_int_bits),
            b"Java_java_lang_Double_doubleToRawLongBits" => {
                into_opaque3ret(double_to_raw_long_bits)
            }
            b"Java_java_lang_Integer_toString" => into_opaque4ret(integer_to_string),
            b"Java_java_lang_Integer_parseInt" => into_opaque4ret(integer_parse_int),
            b"Java_sun_misc_Unsafe_objectFieldOffset" => {
                into_opaque3ret(unsafe_object_field_offset)
            }
            b"Java_sun_misc_Unsafe_getAndAddInt" => into_opaque5ret(unsafe_get_and_add_int),
            _ => return None,
        })
    }
}

extern "C" fn object_get_class(env: *mut Env<'_>, this: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.unwrap();

    let this = env.state.gc.deref(this).unwrap();
    let id = match this {
        Instance::StaticClass(_) => panic!("Should not be static class"),
        Instance::Reference(re) => re.instanceof(),
    };

    let class_form = util::make_class_form_of(env, id, id).unwrap();
    let class_form = match class_form {
        ValueException::Value(class_form) => class_form,
        ValueException::Exception(_) => todo!("There was an exception in Object#getClass"),
    };

    unsafe { env.get_local_jobject_for(class_form.into_generic()) }
}

extern "C" fn object_hashcode(env: *mut Env<'_>, this: JObject) -> JInt {
    // Hashcode impls require that if they're equal then they have the same hashcode
    // So that means the users must override the hashocde if they modify equals
    // And so, since this is for Object, and object's equal is a strict reference equality, we
    // just use the gc index as the value.

    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    if let Some(this) = this {
        let index = this.get_index_unchecked();
        // TODO: Is this fine? It is iffy on 64 bit platforms...
        (index as u32) as i32
    } else {
        // Can this even occur?
        todo!("Null pointer exception")
    }
}

extern "C" fn class_get_primitive(env: *mut Env<'_>, _this: JObject, name: JChar) -> JObject {
    assert!(!env.is_null(), "Env was null when passed into java/lang/Class getPrimitive, which is indicative of an internal bug.");

    let env = unsafe { &mut *env };

    // Note: This assumes that the jchar encoding can be directly compared to the ascii bytes for
    // these basic characters
    let class_name: &[u8] = if name == u16::from(b'B') {
        b"java/lang/Byte"
    } else if name == u16::from(b'C') {
        b"java/lang/Character"
    } else if name == u16::from(b'D') {
        b"java/lang/Double"
    } else if name == u16::from(b'F') {
        b"java/lang/Float"
    } else if name == u16::from(b'I') {
        b"java/lang/Integer"
    } else if name == u16::from(b'J') {
        b"java/lang/Long"
    } else if name == u16::from(b'S') {
        b"java/lang/Short"
    } else if name == u16::from(b'Z') {
        b"java/lang/Bool"
    } else if name == u16::from(b'V') {
        b"java/lang/Void"
    } else {
        panic!("Unknown name ({}) passed into Class#getPrimitive", name);
    };

    let class_id = env.class_names.gcid_from_bytes(class_name);
    let object_id = env.class_names.object_id();

    // We use object_id just to be explicit about them being bootstrap-ish classes
    let class_form = util::make_class_form_of(env, object_id, class_id).expect("Handle errors");
    let class_form = match class_form {
        ValueException::Value(class_form) => class_form,
        ValueException::Exception(exc) => todo!("Handle exceptions"),
    };

    unsafe { env.get_local_jobject_for(class_form.into_generic()) }
}

// TODO: Could we use &mut Env instead, since we know it will call native methods with a non-null
// ptr?
/// java/lang/Class
/// `public Field getDeclaredField(String name);`
extern "C" fn class_get_declared_field(env: *mut Env<'_>, this: JObject, name: JString) -> JObject {
    assert!(!env.is_null(), "Env was null when passed to java/lang/Class getDeclaredField, which is indicative of an internal bug.");

    // SAFETY: We already checked that it is not null, and we rely on native method calling's
    // safety for this to be fine to turn into a reference
    let env = unsafe { &mut *env };

    // Class<T>
    // SAFETY: We assume that it is a valid ref and that it has not been
    // forged.
    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("RegisterNative's class was null");
    let this_id = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        let of = this.of;
        let of = env.state.gc.deref(of).unwrap().id;
        of
    } else {
        // This should be caught by method calling
        // Though it would be good to not panic
        panic!();
    };

    // TODO: null pointer exception
    // SAFETY: We assume that it is a valid ref and that it has not been forged.
    let name = unsafe { env.get_jobject_as_gcref(name) };
    let name = name.expect("getDeclaredField's name was null");
    // TODO: This is doing far more work than needs to be done.
    let name_text = util::get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        name,
    )
    .unwrap();

    let (field_id, field_info) =
        find_field_with_name(&env.class_files, this_id, name_text.as_bytes())
            .unwrap()
            .unwrap();

    let field_class_id = env.class_names.gcid_from_bytes(b"java/lang/reflect/Field");
    let field_internal_class_id = env.class_names.gcid_from_bytes(b"rho/InternalField");

    // TODO: We could make a InternalField a magic class?
    let field_internal_ref = {
        // TODO: resolve derive??
        let field_internal_class_ref = match initialize_class(env, field_internal_class_id)
            .unwrap()
            .into_value()
        {
            ValueException::Value(v) => v,
            ValueException::Exception(_) => todo!(),
        };

        let mut field_internal_fields =
            match make_fields(env, field_internal_class_id, |field_info| {
                !field_info.access_flags.contains(FieldAccessFlags::STATIC)
            })
            .unwrap()
            {
                Either::Left(fields) => fields,
                Either::Right(_exc) => todo!(),
            };

        {
            let (f_class_id, f_field_index) = field_id.decompose();
            let (class_id_field, field_index_field, flags_field) = env
                .state
                .get_internal_field_ids(&env.class_files, field_internal_class_id)
                .unwrap();
            *(field_internal_fields
                .get_mut(class_id_field)
                .unwrap()
                .value_mut()) = RuntimeValuePrimitive::I32(f_class_id.get() as i32).into();
            *(field_internal_fields
                .get_mut(field_index_field)
                .unwrap()
                .value_mut()) = RuntimeValuePrimitive::I16(f_field_index.get() as i16).into();
            *(field_internal_fields
                .get_mut(flags_field)
                .unwrap()
                .value_mut()) =
                RuntimeValuePrimitive::I16(field_info.access_flags.bits() as i16).into();
        };

        let field_internal = ClassInstance {
            instanceof: field_internal_class_id,
            static_ref: field_internal_class_ref,
            fields: field_internal_fields,
        };
        env.state.gc.alloc(field_internal)
    };

    let field_ref = {
        // TODO: resolve derive??
        let field_class_ref = match initialize_class(env, field_class_id).unwrap().into_value() {
            ValueException::Value(v) => v,
            ValueException::Exception(_) => todo!(),
        };

        let mut field_fields = match make_fields(env, field_class_id, |field_info| {
            !field_info.access_flags.contains(FieldAccessFlags::STATIC)
        })
        .unwrap()
        {
            Either::Left(fields) => fields,
            Either::Right(_exc) => todo!(),
        };

        {
            let internal_field_id = env
                .state
                .get_field_internal_field_id(&env.class_files, field_class_id)
                .unwrap();
            *(field_fields.get_mut(internal_field_id).unwrap().value_mut()) =
                RuntimeValue::Reference(field_internal_ref.into_generic());
        };

        let field = ClassInstance {
            instanceof: field_class_id,
            static_ref: field_class_ref,
            fields: field_fields,
        };
        env.state.gc.alloc(field)
    };

    unsafe { env.get_local_jobject_for(field_ref.into_generic()) }
}

/// java/lang/System
/// `public static void arraycopy(Object src, int src_pos, Object dest, int dest_position, int
/// length)`
extern "C" fn system_arraycopy(
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
            (ReferenceInstance::ReferenceArray(_), ReferenceInstance::ReferenceArray(_)) => todo!(),
            (ReferenceInstance::PrimitiveArray(_), _)
            | (_, ReferenceInstance::PrimitiveArray(_)) => todo!("Wrong types"),
            (ReferenceInstance::ReferenceArray(_), _)
            | (_, ReferenceInstance::ReferenceArray(_)) => todo!("Wrong types"),
            _ => panic!("Throw exception, this should be an array"),
        },
    };
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

extern "C" fn float_to_raw_int_bits(_env: *mut Env<'_>, _this: JObject, value: JFloat) -> JInt {
    i32::from_be_bytes(value.to_be_bytes())
}

extern "C" fn double_to_raw_long_bits(_env: *mut Env<'_>, _this: JObject, value: JDouble) -> JLong {
    i64::from_be_bytes(value.to_be_bytes())
}

// TODO: Is this correct for hex/binary/octal in java's integer class?
extern "C" fn integer_to_string(
    env: *mut Env<'_>,
    _this: JObject,
    val: JInt,
    radix: JInt,
) -> JString {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    if !(2..=36).contains(&radix) {
        todo!("Exception, radix was out of bounds");
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let radix = radix as u8;

    let result = radix_fmt::radix(val, radix as u8);
    // java uses lowercase for this
    let result = format!("{}", result);
    let result = result
        .encode_utf16()
        .map(JavaChar)
        .map(RuntimeValuePrimitive::Char)
        .collect::<Vec<_>>();

    let string = util::construct_string(env, result).expect("Failed to create string");
    let string = match string {
        ValueException::Value(string) => string,
        ValueException::Exception(_) => {
            todo!("There was an exception converting integer to string")
        }
    };

    unsafe { env.get_local_jobject_for(string.into_generic()) }
}

extern "C" fn integer_parse_int(
    env: *mut Env<'_>,
    _this: JObject,
    source: JString,
    radix: JInt,
) -> JInt {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    if !(2..=36).contains(&radix) {
        todo!("Exception, radix was out of bounds");
    }

    let radix = radix.unsigned_abs();

    let source = unsafe { env.get_jobject_as_gcref(source) };
    let source = source.expect("null source ref");
    let source = util::get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        source,
    )
    .unwrap();

    // TODO: We could do this manually ourselves directly from the utf16 string, which would be
    // faster than converting it to a rust string and back..
    // TODO: Does this match java's behavior?
    i32::from_str_radix(&source, radix).expect("Failed to parse integer")
}

/// sun/misc/Unsafe
/// `public long objectFieldOffset(Field field);`
/// This just returns a unique id
extern "C" fn unsafe_object_field_offset(
    env: *mut Env<'_>,
    _this: JObject,
    field_ref: JObject,
) -> JLong {
    assert!(!env.is_null(), "Env was null when passed to sun/misc/Unsafe objectFieldOffset, which is indicative of an internal bug.");

    // SAFETY: We already checked that it is not null, and we rely on native method calling's
    // safety for this to be fine to turn into a reference
    let env = unsafe { &mut *env };

    let field_ref = unsafe { env.get_jobject_as_gcref(field_ref) };
    // TODO: null pointer exception
    let field_ref = field_ref.expect("objectFieldOffset's field ref was null");

    let field_class_id = env.class_names.gcid_from_bytes(b"java/lang/reflect/Field");
    let field_internal_class_id = env.class_names.gcid_from_bytes(b"rho/InternalField");
    let internal_field_id = env
        .state
        .get_field_internal_field_id(&env.class_files, field_class_id)
        .unwrap();
    let (class_id_field, field_index_field, _) = env
        .state
        .get_internal_field_ids(&env.class_files, field_internal_class_id)
        .unwrap();

    let field = env.state.gc.deref(field_ref).unwrap();
    let field = if let Instance::Reference(ReferenceInstance::Class(field)) = field {
        field
    } else {
        panic!("Bad field reference");
    };
    let internal_field_ref = field
        .fields
        .get(internal_field_id)
        .unwrap()
        .value()
        .into_reference()
        .expect("internal field should be a reference")
        .expect("Got null ptr for internal field");

    let internal_field = env.state.gc.deref(internal_field_ref).unwrap();
    let internal_field = if let ReferenceInstance::Class(field) = internal_field {
        field
    } else {
        panic!("Bad internal field reference");
    };

    let class_id_val = internal_field
        .fields
        .get(class_id_field)
        .expect("class id field should exist")
        .value()
        .into_i32()
        .expect("classid field should be i32");
    let class_id_val = ClassId::new_unchecked(class_id_val as u32);

    let field_index_val = internal_field
        .fields
        .get(field_index_field)
        .expect("field index field should exist")
        .value()
        .into_i16()
        .expect("field index field should be i16");
    let field_index_val = FieldIndex::new_unchecked(field_index_val as u16);

    // Safety: Only the JVM should fill out the Field class and so the values should be valid
    let field_id = unsafe { JFieldId::new_unchecked(class_id_val, field_index_val) };
    field_id.as_i64()
}

/// sun/misc/Unsafe
/// `int getAndAddInt(Object src, long offset, int delta);`
extern "C" fn unsafe_get_and_add_int(
    env: *mut Env<'_>,
    _this: JObject,
    target: JObject,
    offset: JLong,
    add_val: JInt,
) -> JInt {
    assert!(!env.is_null(), "Env was null when passed to sun/misc/Unsafe objectFieldOffset, which is indicative of an internal bug.");

    // SAFETY: We already checked that it is not null, and we rely on native method calling's
    // safety for this to be fine to turn into a reference
    let env = unsafe { &mut *env };

    let target = unsafe { env.get_jobject_as_gcref(target) };
    // We don't have to validate, since method calling should have done that
    let target: GcRef<ClassInstance> = target.expect("Null pointer exception").unchecked_as();

    let field_id = JFieldId::unchecked_from_i64(offset);
    let field_id = unsafe { field_id.into_field_id() };
    let field_id = field_id.expect("Field id was null");

    // FIXME: This is meant to be atomic!

    let target = env.state.gc.deref_mut(target).unwrap();
    // TODO: exception
    let target_val = target
        .fields
        .get_mut(field_id)
        .expect("Field offset doesn't exist for this field")
        .value_mut();
    let current_val = (*target_val).into_i32().expect("Field value should be int");

    *target_val = RuntimeValuePrimitive::I32(current_val.overflowing_add(add_val).0).into();

    current_val
}
