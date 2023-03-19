use crate::{
    class_instance::{Instance, MethodHandleType, ReferenceInstance},
    eval::class_util::get_init_method_type_from_mh,
    jni::{JClass, JInt, JObject},
    util::{construct_string_r, make_class_form_of, Env},
};

pub(crate) extern "C" fn mh_info_get_declaring_class(env: *mut Env, this: JObject) -> JClass {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Null reference");
    let Some(Instance::Reference(ReferenceInstance::MethodHandleInfo(mh_info))) = env.state.gc.deref(this) else {
        unreachable!()
    };

    let method_handle_ref = mh_info.method_handle;
    let Some(method_handle) = env.state.gc.deref(method_handle_ref) else {
        unreachable!()
    };

    // Get the class the method was declared on
    let class_id = match &method_handle.typ {
        // Non direct method handle, so it shouldn't be constructable
        MethodHandleType::Constant { .. } => unreachable!(),
        MethodHandleType::InvokeStatic(method_id)
        | MethodHandleType::InvokeSpecial(method_id)
        | MethodHandleType::NewInvokeSpecial(method_id)
        | MethodHandleType::InvokeInterface(method_id) => method_id.decompose().0,
    };

    // TODO: using the same id for this is bad
    let class_form = make_class_form_of(env, class_id, class_id).unwrap();
    let Some(class_form) = env.state.extract_value(class_form) else {
        return JClass::null();
    };
    let class_form = class_form.into_generic();

    unsafe { env.get_local_jobject_for(class_form) }
}

pub(crate) extern "C" fn mh_info_get_method_type(env: *mut Env, this: JObject) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Null reference");

    if let Instance::Reference(ReferenceInstance::MethodHandleInfo(mh_info)) =
        env.state.gc.deref(this).unwrap()
    {
        let mh = mh_info.method_handle;
        let method_type =
            get_init_method_type_from_mh(env, mh).expect("Failed to construct MethodType instance");
        let Some(method_type) = env.state.extract_value(method_type) else {
            // exception
            return JObject::null();
        };

        unsafe { env.get_local_jobject_for(method_type.into_generic()) }
    } else {
        unreachable!()
    }
}

pub(crate) extern "C" fn mh_info_get_reference_kind(env: *mut Env, this: JObject) -> JInt {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Null reference");
    if let Instance::Reference(ReferenceInstance::MethodHandleInfo(mh_info)) =
        env.state.gc.deref(this).unwrap()
    {
        let mh = mh_info.method_handle;
        let mh = env.state.gc.deref(mh).unwrap();

        if let Some(kind) = mh.typ.direct_kind() {
            kind.into()
        } else {
            todo!("IllegalArgumentException due to not being a direct method handle")
        }
    } else {
        unreachable!()
    }
}

pub(crate) extern "C" fn mh_info_get_name(env: *mut Env, this: JObject) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Null reference");
    let Some(Instance::Reference(ReferenceInstance::MethodHandleInfo(mh_info))) =
        env.state.gc.deref(this) else { unreachable!() };

    let mh = mh_info.method_handle;
    let Some(mh) = env.state.gc.deref(mh) else { unreachable!() };

    let method_id = match &mh.typ {
        // Non direct method handle, so it shouldn't be constructable
        MethodHandleType::Constant { .. } => unreachable!(),
        MethodHandleType::InvokeStatic(method_id)
        | MethodHandleType::InvokeSpecial(method_id)
        | MethodHandleType::NewInvokeSpecial(method_id)
        | MethodHandleType::InvokeInterface(method_id) => method_id,
    };

    let (class_id, _) = method_id.decompose();
    let class = env.class_files.get(&class_id).unwrap();
    // TODO: We should probably load the method
    let Some(method) = env.methods.get(method_id) else {
        unreachable!()
    };
    let method_name = class.getr_text(method.name_index()).unwrap().into_owned();

    let text = construct_string_r(env, &method_name, true).unwrap();
    let Some(text) = env.state.extract_value(text) else {
        return JObject::null();
    };

    unsafe { env.get_local_jobject_for(text.into_generic()) }
}
