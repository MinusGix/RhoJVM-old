//! Internal replacements for native functions  

use classfile_parser::field_info::FieldAccessFlags;
use either::Either;

use crate::{
    class_instance::{ClassInstance, Instance, ReferenceInstance},
    eval::{instances::make_fields, EvalError, ValueException},
    initialize_class,
    jni::{JObject, JString, JValue, MethodClassNoArguments, OpaqueClassMethod},
    rv::{RuntimeType, RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    util::{find_field_with_name, Env},
};

// TODO: Should we use something like PHF? Every native lookup is going to check this array
// for if it exists, which does make them all more expensive for this case. PHF would probably be
// faster than whatever llvm optimizes this to.

/// Converts function ptr into opaque method ptr for use by native calling code
/// # Safety
unsafe fn into_opaque3ret<R>(
    f: unsafe extern "C" fn(*mut Env<'_>, JObject, JObject) -> R,
) -> OpaqueClassMethod {
    OpaqueClassMethod::new(std::mem::transmute::<
        unsafe extern "C" fn(*mut Env<'_>, JObject, JObject) -> R,
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
    unsafe {
        Some(match name {
            b"Java_java_lang_Class_getDeclaredField" => into_opaque3ret(class_get_declared_field),
            _ => return None,
        })
    }
}

// TODO: Could we use &mut Env instead, since we know it will call native methods with a non-null
// ptr?
/// java/lang/Class
/// `public Field getDeclaredField(String name);`
extern "C" fn class_get_declared_field(env: *mut Env<'_>, this: JObject, name: JString) -> JValue {
    assert!(!env.is_null(), "Env was null when passed to java/lang/Class getDeclaredField, which is indicative of an internal bug.");

    // SAFETY: We already checked that it is not null, and we rely on native method calling's
    // safety for this to be fine to turn into a reference
    let env = unsafe { &mut *env };

    // Get the string field id information, since we need it to extract the value inside name
    let string_id = env.class_names.gcid_from_bytes(b"java/lang/String");
    let string_content_field = env
        .state
        .get_string_data_field(&env.class_files, string_id)
        .expect("getDeclaredField failed to get data field id for string");

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
    let name_text = {
        // TODO: Don't unwrap
        let name = match env
            .state
            .gc
            .deref(name)
            .ok_or(EvalError::InvalidGcRef(name))
            .unwrap()
        {
            Instance::StaticClass(_) => panic!("Got static class gcref for String"),
            Instance::Reference(v) => match v {
                ReferenceInstance::Class(v) => v,
                _ => panic!("Did not get normal Class gcref for String"),
            },
        };

        // We don't have to verify that name is of the right class because the function calling
        // code would verify that it is being passed a string.
        // but also, String is final

        let data = name
            .fields
            .get(string_content_field)
            .ok_or(EvalError::MissingField(string_content_field))
            .expect("getDeclaredField failed to get data field from string name");

        let data = data.value();
        let data = data
            .into_reference()
            .expect("string data field to be a reference")
            .expect("string data field to be non-null");

        let data = match env.state.gc.deref(data).unwrap() {
            ReferenceInstance::PrimitiveArray(arr) => arr,
            _ => panic!("Bad type for name text"),
        };
        assert_eq!(data.element_type, RuntimeTypePrimitive::Char);
        // Converting back to cesu8 is expensive, but this kind of operation isn't common enough to do
        // anything like storing cesu8 versions alongside them, probably.
        let data = data
            .elements
            .iter()
            .map(|x| x.into_char().unwrap().0)
            .collect::<Vec<u16>>();
        // TODO: Convert to cesu8. This is currently incorrect.
        String::from_utf16(&data).unwrap()
    };

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
                .get_mut(field_index_field)
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

    let field_ref_jobject = unsafe { env.get_local_jobject_for(field_ref.into_generic()) };
    JValue {
        l: field_ref_jobject,
    }
}
