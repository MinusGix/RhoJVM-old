use classfile_parser::{field_info::FieldAccessFlags, ClassAccessFlags};
use either::Either;
use rhojvm_base::{code::method::MethodDescriptor, id::ClassId};

use crate::{
    class_instance::{ClassInstance, Instance, ReferenceInstance},
    eval::{eval_method, instances::make_fields, EvalMethodValue, Frame, Locals, ValueException},
    gc::GcRef,
    initialize_class,
    jni::{JBoolean, JChar, JObject, JString},
    rv::{RuntimeValue, RuntimeValuePrimitive},
    util::{self, find_field_with_name, Env},
    GeneralError,
};

pub(crate) extern "C" fn class_get_primitive(
    env: *mut Env<'_>,
    _this: JObject,
    name: JChar,
) -> JObject {
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

pub(crate) extern "C" fn class_get_class_for_name_with_class_loader(
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

pub(crate) extern "C" fn class_get_class_for_name(
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
pub(crate) extern "C" fn class_get_declared_field(
    env: *mut Env<'_>,
    this: JObject,
    name: JString,
) -> JObject {
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

pub(crate) extern "C" fn class_new_instance(env: *mut Env<'_>, this: JObject) -> JObject {
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
