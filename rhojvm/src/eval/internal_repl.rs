//! Internal replacements for native functions  

use classfile_parser::{field_info::FieldAccessFlags, ClassAccessFlags};
use either::Either;
use rhojvm_base::{
    code::{method::MethodDescriptor, types::JavaChar},
    id::ClassId,
};
use usize_cast::IntoUsize;

use crate::{
    class_instance::{
        ClassInstance, FieldIndex, Instance, PrimitiveArrayInstance, ReferenceInstance,
    },
    eval::{eval_method, instances::make_fields, EvalMethodValue, Frame, Locals, ValueException},
    gc::GcRef,
    initialize_class,
    jni::{
        JBoolean, JByte, JChar, JDouble, JFieldId, JFloat, JInt, JLong, JObject, JShort, JString,
        MethodClassNoArguments, OpaqueClassMethod,
    },
    memblock::MemoryBlockPtr,
    rv::{RuntimeValue, RuntimeValuePrimitive},
    util::{self, find_field_with_name, Env},
    GeneralError,
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
            // SystemClassLoader
            b"Java_rho_SystemClassLoader_initializeSystemClassLoader" => {
                into_opaque2ret(system_class_loader_init)
            }
            // Object
            b"Java_java_lang_Object_getClass" => into_opaque2ret(object_get_class),
            b"Java_java_lang_Object_hashCode" => into_opaque2ret(object_hashcode),
            // Class
            b"Java_java_lang_Class_getPrimitive" => into_opaque3ret(class_get_primitive),
            b"Java_java_lang_Class_getClassForNameWithClassLoader" => {
                into_opaque5ret(class_get_class_for_name_with_class_loader)
            }
            b"Java_java_lang_Class_getClassForName" => into_opaque3ret(class_get_class_for_name),
            b"Java_java_lang_Class_getDeclaredField" => into_opaque3ret(class_get_declared_field),
            b"Java_java_lang_Class_newInstance" => into_opaque2ret(class_new_instance),
            // System
            b"Java_java_lang_System_arraycopy" => into_opaque7ret(system_arraycopy),
            // Primitive wrappers
            b"Java_java_lang_Float_floatToRawIntBits" => into_opaque3ret(float_to_raw_int_bits),
            b"Java_java_lang_Double_doubleToRawLongBits" => {
                into_opaque3ret(double_to_raw_long_bits)
            }
            b"Java_java_lang_Integer_toString" => into_opaque4ret(integer_to_string),
            b"Java_java_lang_Integer_parseInt" => into_opaque4ret(integer_parse_int),
            // Unsafe allocation
            b"Java_sun_misc_Unsafe_allocateMemory" => into_opaque3ret(unsafe_allocate_memory),
            b"Java_sun_misc_Unsafe_freeMemory" => into_opaque3ret(unsafe_free_memory),
            // Unsafe get
            b"Java_sun_misc_Unsafe_getByte" => into_opaque3ret(unsafe_get_byte),
            b"Java_sun_misc_Unsafe_getShort" => into_opaque3ret(unsafe_get_short),
            b"Java_sun_misc_Unsafe_getChar" => into_opaque3ret(unsafe_get_char),
            b"Java_sun_misc_Unsafe_getInt" => into_opaque3ret(unsafe_get_int),
            b"Java_sun_misc_Unsafe_getLong" => into_opaque3ret(unsafe_get_long),
            b"Java_sun_misc_Unsafe_getFloat" => into_opaque3ret(unsafe_get_float),
            b"Java_sun_misc_Unsafe_getDouble" => into_opaque3ret(unsafe_get_double),
            // Unsafe put
            b"Java_sun_misc_Unsafe_putByte" => into_opaque4ret(unsafe_put_byte),
            b"Java_sun_misc_Unsafe_putShort" => into_opaque4ret(unsafe_put_short),
            b"Java_sun_misc_Unsafe_putChar" => into_opaque4ret(unsafe_put_char),
            b"Java_sun_misc_Unsafe_putInt" => into_opaque4ret(unsafe_put_int),
            b"Java_sun_misc_Unsafe_putLong" => into_opaque4ret(unsafe_put_long),
            b"Java_sun_misc_Unsafe_putFloat" => into_opaque4ret(unsafe_put_float),
            b"Java_sun_misc_Unsafe_putDouble" => into_opaque4ret(unsafe_put_double),
            // Unsafe fields
            b"Java_sun_misc_Unsafe_objectFieldOffset" => {
                into_opaque3ret(unsafe_object_field_offset)
            }
            b"Java_sun_misc_Unsafe_getAndAddInt" => into_opaque5ret(unsafe_get_and_add_int),
            _ => return None,
        })
    }
}

extern "C" fn system_class_loader_init(env: *mut Env<'_>, _this: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let scl_id = env.class_names.gcid_from_bytes(b"rho/SystemClassLoader");

    // We're probably in this already.
    let scl_ref = initialize_class(env, scl_id).unwrap().into_value();
    let scl_ref = match scl_ref {
        ValueException::Value(re) => re,
        ValueException::Exception(_) => {
            todo!("There was an exception when initializing the System ClassLoader class")
        }
    };

    let fields = match make_fields(env, scl_id, |field_info| {
        !field_info.access_flags.contains(FieldAccessFlags::STATIC)
    })
    .unwrap()
    {
        Either::Left(fields) => fields,
        Either::Right(_) => {
            todo!("There was an exception when initializing the System ClassLoader's fields")
        }
    };

    let inst = ClassInstance {
        instanceof: scl_id,
        static_ref: scl_ref,
        fields,
    };

    let inst_ref = env.state.gc.alloc(inst);

    unsafe { env.get_local_jobject_for(inst_ref.into_generic()) }
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

/// Gets the class name id for a slice of java characters in the format of Class's name
/// This is basically the same as typical, except instead of / it uses .
fn get_class_name_id_for(env: &mut Env, name: GcRef<Instance>) -> Result<ClassId, GeneralError> {
    // TODO: We could do cheaper insertion, especially if it already exists in class names
    let contents =
        util::get_string_contents(&env.class_files, &mut env.class_names, &mut env.state, name)?;
    // Converting back to cesu8 is expensive, but this kind of operation isn't common enough to do
    // anything like storing cesu8 versions alongside them, probably.
    let contents = contents
        .iter()
        .map(|x| x.into_char().unwrap().0)
        .map(|x| {
            if x == u16::from(b'.') {
                u16::from(b'/')
            } else {
                x
            }
        })
        .collect::<Vec<u16>>();
    let contents = String::from_utf16(&contents).map_err(GeneralError::StringConversionFailure)?;
    tracing::info!("Get Class Name Id for: {}", contents);
    // TODO: We should actually convert it to cesu8!
    let contents = contents.as_bytes();
    let id = env.class_names.gcid_from_bytes(contents);
    tracing::info!("\tId: {:?}", id);
    Ok(id)
}

extern "C" fn class_get_class_for_name_with_class_loader(
    env: *mut Env<'_>,
    _this: JObject,
    name: JString,
    _initialize: JBoolean,
    _loader: JObject,
) -> JObject {
    // FIXME: We're currently ignoring the loader

    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let name = unsafe { env.get_jobject_as_gcref(name) };
    let name = name.expect("null ref exception");

    let class_id = get_class_name_id_for(env, name).unwrap();

    // FIXME: I believe this is wrong, however our current implementation requires the class to be
    // initialized before a Class<?> can be made for it, since it requires a StaticClassInstance.
    // We likely have to loosen that to only having the class file be loaded.
    // The make class form of will always initialize it
    // FIXME: I think we should use the caller here? Or modify it so it can take the loader?
    let class_form = util::make_class_form_of(env, class_id, class_id).expect("Handle errors");
    let class_form = match class_form {
        ValueException::Value(form) => form,
        ValueException::Exception(_) => todo!("Exception in the creating class form"),
    };

    unsafe { env.get_local_jobject_for(class_form.into_generic()) }
}

extern "C" fn class_get_class_for_name(
    env: *mut Env<'_>,
    _this: JObject,
    name: JString,
) -> JObject {
    todo!()
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

extern "C" fn class_new_instance(env: *mut Env<'_>, this: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Class new instance's this ref was null");
    // The id held inside
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

    let static_ref = initialize_class(env, this_id).unwrap().into_value();
    let static_ref = match static_ref {
        ValueException::Value(static_ref) => static_ref,
        ValueException::Exception(_) => todo!("Handle exception in initializing class"),
    };

    let target_class = env.classes.get(&this_id).unwrap();
    if target_class.is_interface()
        || target_class
            .access_flags()
            .contains(ClassAccessFlags::ABSTRACT)
    {
        todo!("InstantiationError exception");
    }

    let fields = match make_fields(env, this_id, |field_info| {
        !field_info.access_flags.contains(FieldAccessFlags::STATIC)
    })
    .unwrap()
    {
        Either::Left(fields) => fields,
        Either::Right(_) => {
            todo!("Exception in making fields for class")
        }
    };

    let class = ClassInstance {
        instanceof: this_id,
        static_ref,
        fields,
    };

    // Allocate the class instance
    let class_ref = env.state.gc.alloc(class);

    // However, now we have to run the empty constructor if one exists for the class.
    // Note: We don't include the `this` pointer because that is implicit in the descriptor
    let descriptor = MethodDescriptor::new_empty();

    // TODO: We need to check that they can access the constructor!
    let method_id = env
        .methods
        .load_method_from_desc(
            &env.class_directories,
            &mut env.class_names,
            &mut env.class_files,
            this_id,
            b"<init>",
            &descriptor,
        )
        .unwrap();

    let locals = Locals::new_with_array([RuntimeValue::Reference(class_ref.into_generic())]);
    let frame = Frame::new_locals(locals);
    match eval_method(env, method_id, frame).unwrap() {
        EvalMethodValue::ReturnVoid => {}
        EvalMethodValue::Return(_) => tracing::warn!("Constructor returned value?"),
        EvalMethodValue::Exception(_) => {
            todo!("There was an exception calling the default constructor")
        }
    }

    // Now we just return the initialized class ref
    unsafe { env.get_local_jobject_for(class_ref.into_generic()) }
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

type JAddress = JLong;

extern "C" fn unsafe_allocate_memory(env: *mut Env<'_>, _this: JObject, size: JLong) -> JAddress {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let size: usize = size.try_into().expect("Out of memory error");

    let ptr = env.state.mem_blocks.allocate_block(size).unwrap();
    let ptr = ptr.get();
    let ptr = ptr as usize;
    let ptr: JAddress = ptr
        .try_into()
        .expect("Address was too large to fit into a long");

    ptr
}

unsafe fn conv_address(address: JAddress) -> MemoryBlockPtr {
    let address: usize = address.try_into().expect("Address was too high to fit into a usize. This is probably indicative of a bug in the Java code or internally.");
    let address = address as *mut u8;
    // Safety: We basically have to trust it to be valid.
    MemoryBlockPtr::new_unchecked(address)
}

extern "C" fn unsafe_free_memory(env: *mut Env<'_>, _this: JObject, address: JAddress) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We are only deallocating, so it just checks if the pointer exists in our allocations.
    // So not really unsafe.
    let address = unsafe { conv_address(address) };

    env.state.mem_blocks.deallocate_block(address);
}

extern "C" fn unsafe_get_byte(env: *mut Env<'_>, _this: JObject, address: JAddress) -> JByte {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_i8_ne(address) }
}

extern "C" fn unsafe_get_short(env: *mut Env<'_>, _this: JObject, address: JAddress) -> JShort {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_i16_ne(address) }
}

extern "C" fn unsafe_get_char(env: *mut Env<'_>, _this: JObject, address: JAddress) -> JChar {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_u16_ne(address) }
}

extern "C" fn unsafe_get_int(env: *mut Env<'_>, _this: JObject, address: JAddress) -> JInt {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_i32_ne(address) }
}

extern "C" fn unsafe_get_long(env: *mut Env<'_>, _this: JObject, address: JAddress) -> JLong {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_i64_ne(address) }
}

extern "C" fn unsafe_get_float(env: *mut Env<'_>, _this: JObject, address: JAddress) -> JFloat {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_f32_ne(address) }
}

extern "C" fn unsafe_get_double(env: *mut Env<'_>, _this: JObject, address: JAddress) -> JDouble {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_f64_ne(address) }
}

extern "C" fn unsafe_put_byte(env: *mut Env<'_>, _this: JObject, address: JAddress, value: JByte) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_i8_ne(address, value) };
}

extern "C" fn unsafe_put_short(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
    value: JShort,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_i16_ne(address, value) };
}

extern "C" fn unsafe_put_char(env: *mut Env<'_>, _this: JObject, address: JAddress, value: JChar) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_u16_ne(address, value) };
}

extern "C" fn unsafe_put_int(env: *mut Env<'_>, _this: JObject, address: JAddress, value: JInt) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_i32_ne(address, value) };
}

extern "C" fn unsafe_put_long(env: *mut Env<'_>, _this: JObject, address: JAddress, value: JLong) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_i64_ne(address, value) };
}

extern "C" fn unsafe_put_float(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
    value: JFloat,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_f32_ne(address, value) };
}

extern "C" fn unsafe_put_double(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
    value: JDouble,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_f64_ne(address, value) };
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
